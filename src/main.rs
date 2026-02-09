mod actions;
mod api;
mod cli;
mod config;
mod error;
mod filter;
mod hooks;
mod markdown;
mod models;
mod notifications;
mod preview;
mod preview_manager;
mod state;
mod state_file;
mod terminal;
mod ui;

use clap::Parser;
use cli::Args;
use config::Config;
use error::Result;
use filter::Filter;
use notifications::{fetch_notifications, NotificationFetchOptions};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use terminal::Terminal;
use ui::{App, InitialLoadData, PendingStateSettings};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Merged runtime options from CLI args and config file.
#[derive(Clone)]
struct RuntimeOptions {
    show_all: bool,
    participating: bool,
    max_notifications: Option<usize>,
    filter_pattern: Option<String>,
    auto_mark_read: bool,
    auto_archive: bool,
}

impl RuntimeOptions {
    fn from_args_and_config(args: &Args, config: &Config) -> Self {
        let auto_archive = !args.no_auto_archive && config.auto_archive;
        // When auto_archive is enabled, auto_mark_read is implicitly forced on
        let auto_mark_read = auto_archive || (!args.no_auto_mark_read && config.auto_mark_read);
        Self {
            // CLI flags override config (for booleans, CLI true wins)
            show_all: args.all || config.show_read,
            participating: args.participating || config.participating_only,
            // CLI takes precedence if provided
            max_notifications: args.max_notifications.or(config.max_notifications),
            filter_pattern: args.filter.clone().or(config.default_filter.clone()),
            auto_mark_read,
            auto_archive,
        }
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(args.config.as_deref());
    let opts = RuntimeOptions::from_args_and_config(&args, &config);

    // Initialise state file path: CLI > config > default
    let state_path = args
        .state_file
        .or_else(|| config.state_file.as_ref().map(PathBuf::from));
    state_file::init_state_path(state_path)?;

    // Handle non-interactive modes first
    if args.mark_read || args.mark_read_archive {
        let client = api::GitHubClient::new(&config)?;
        let archive = args.mark_read_archive;

        let notifications = fetch_notifications(
            &client,
            NotificationFetchOptions {
                show_all: opts.show_all,
                participating: opts.participating,
                max_notifications: opts.max_notifications,
                per_page: config.pagination_size,
            },
        )?;

        let filter = build_filter(&opts.filter_pattern)?;
        let to_process: Vec<_> = match filter {
            Some(ref filter) => notifications.iter().filter(|n| filter.matches(n)).collect(),
            None => notifications.iter().collect(),
        };

        // Use bulk API when marking all as read (no filter, no archive)
        if !archive && opts.filter_pattern.is_none() {
            client.mark_all_read(None)?;
        } else {
            for notif in &to_process {
                if archive {
                    let _ = client.mark_thread_done(&notif.id);
                } else {
                    let _ = client.mark_notification_read(&notif.id);
                }
            }
        }

        let action = if archive { "read and archived" } else { "read" };
        if opts.filter_pattern.is_some() {
            println!(
                "Marked {} filtered notifications as {}.",
                to_process.len(),
                action
            );
        } else {
            println!("Marked {} notifications as {}.", to_process.len(), action);
        }
        return Ok(());
    }

    if args.static_display {
        handle_static_display(&config, &opts)?;
        return Ok(());
    }

    // Interactive mode
    let client = api::GitHubClient::new(&config)?;
    let mut terminal = Terminal::new()?;
    let mut app = App::new(config.clone());

    // Set API client in app for fetching previews
    app.set_api_client(client.clone());

    // Start auto-refresh if enabled
    app.start_auto_refresh(opts.show_all, opts.participating, opts.max_notifications);

    // Apply filters
    let filter = build_filter(&opts.filter_pattern)?;

    // Always use config's default preview mode on startup
    let preview_mode = config.get_default_preview_mode();

    // Load auto-mark-read and auto-archive settings, with persisted state taking precedence.
    let auto_mark_read =
        state_file::AppStateFile::load_auto_mark_read().unwrap_or(opts.auto_mark_read);
    let auto_archive = state_file::AppStateFile::load_auto_archive().unwrap_or(opts.auto_archive);

    // Set initial values. `set_auto_archive` will correctly enforce `auto_mark_read` if needed.
    app.set_auto_mark_read(auto_mark_read);
    app.set_auto_archive(auto_archive);

    let settings = PendingStateSettings {
        filter,
        filter_pattern: opts.filter_pattern.clone(),
        show_all: opts.show_all,
        repos_collapsed: config.repos_collapsed,
        preview_mode,
    };

    let (tx, rx) = channel();
    let config_clone = config.clone();
    let opts_clone = opts.clone();
    let client_clone = client.clone();
    thread::spawn(move || {
        let result = (|| {
            let mut notifications = fetch_notifications(
                &client_clone,
                NotificationFetchOptions {
                    show_all: opts_clone.show_all,
                    participating: opts_clone.participating,
                    max_notifications: opts_clone.max_notifications,
                    per_page: config_clone.pagination_size,
                },
            )?;
            let pinned_notifications =
                state_file::AppStateFile::load_pinned_notifications().unwrap_or_default();
            for pinned in &pinned_notifications {
                if !notifications.iter().any(|n| n.id == pinned.id) {
                    notifications.push(pinned.clone());
                }
            }
            Ok(InitialLoadData {
                notifications,
                pinned_notifications,
            })
        })();
        let _ = tx.send(result);
    });

    app.start_initial_load(rx, settings);

    // Run the app
    app.run(&mut terminal)?;

    Ok(())
}

fn handle_static_display(config: &Config, opts: &RuntimeOptions) -> Result<()> {
    let client = api::GitHubClient::new(config)?;
    let notifications = fetch_notifications(
        &client,
        NotificationFetchOptions {
            show_all: opts.show_all,
            participating: opts.participating,
            max_notifications: opts.max_notifications,
            per_page: config.pagination_size,
        },
    )?;

    let filter = build_filter(&opts.filter_pattern)?;

    for notification in &notifications {
        if let Some(ref filter) = filter {
            if !filter.matches(notification) {
                continue;
            }
        }

        let (owner, name) = notification.repo_abbreviated();
        let time = notification.time_display();
        let unread_symbol = if notification.is_unread() { "•" } else { " " };

        println!(
            "{} {} {}/{} {} {} {}",
            unread_symbol,
            time,
            owner,
            name,
            notification.notification_type(),
            notification.reason_enum(),
            notification.title()
        );
    }

    if notifications.is_empty() {
        println!("All caught up!");
    }

    Ok(())
}

fn build_filter(pattern: &Option<String>) -> Result<Option<Filter>> {
    match pattern.as_deref() {
        Some(value) => Ok(Some(Filter::new(Some(value))?)),
        None => Ok(None),
    }
}
