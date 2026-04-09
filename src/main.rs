mod actions;
mod api;
mod builtin_actions;
mod builtin_views;
mod cache;
mod cli;
mod config;
mod error;
mod events;
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
mod workflow_runs;

use clap::Parser;
use cli::Args;
use config::Config;
use error::Result;
use filter::Filter;
use notifications::{fetch_extra_sources, fetch_notifications, NotificationFetchOptions};
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
}

impl RuntimeOptions {
    fn from_args_and_config(args: &Args, config: &Config) -> Self {
        Self {
            // CLI flags override config (for booleans, CLI true wins)
            show_all: args.all || config.show_read,
            participating: args.participating || config.participating_only,
            // CLI takes precedence if provided
            max_notifications: args.max_notifications.or(config.max_notifications),
            filter_pattern: args.filter.clone().or(config.default_filter.clone()),
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

        let filter = Filter::from_config(opts.filter_pattern.as_deref(), &config)?;
        let to_process: Vec<_> = notifications.iter().filter(|n| filter.matches(n)).collect();

        // Use bulk API when marking all as read (no filter, no archive)
        if !archive
            && opts.filter_pattern.is_none()
            && config.exclude_types.is_empty()
            && config.exclude_reasons.is_empty()
            && config.exclude_repos.is_empty()
            && config.exclude_subjects.is_empty()
        {
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
    let filter = opts
        .filter_pattern
        .as_deref()
        .map(|pattern| Filter::from_config(Some(pattern), &config))
        .transpose()?
        .or_else(|| {
            // Even without a regex pattern, apply structured excludes if configured
            if config.exclude_types.is_empty()
                && config.exclude_reasons.is_empty()
                && config.exclude_repos.is_empty()
                && config.exclude_subjects.is_empty()
            {
                None
            } else {
                Filter::from_config(None, &config).ok()
            }
        });

    // Always use config's default preview mode on startup
    let preview_mode = config.get_default_preview_mode();

    // Config disabling always wins; otherwise use the persisted toggle (m-key).
    let auto_mark_read = if !config.auto_mark_read {
        false
    } else {
        state_file::AppStateFile::load_auto_mark_read().unwrap_or(true)
    };
    let auto_archive = if !config.auto_archive {
        false
    } else {
        state_file::AppStateFile::load_auto_archive().unwrap_or(false)
    };

    // Set initial values. `set_auto_archive` will correctly enforce `auto_mark_read` if needed.
    app.set_auto_mark_read(auto_mark_read);
    app.set_auto_archive(auto_archive);
    app.set_auto_mark_on_open(config.auto_mark_on_open);

    let settings = PendingStateSettings {
        filter,
        filter_pattern: opts.filter_pattern.clone(),
        show_all: opts.show_all,
        repos_collapsed: config.repos_collapsed,
        preview_mode,
    };

    // Notification cache setup
    let use_cache = !args.no_cache && !args.static_display;
    let options_hash =
        cache::compute_options_hash(opts.show_all, opts.participating, opts.max_notifications);
    let cache_path = cache::get_cache_path(config.cache_file.as_deref())?;

    // Pass cache info to app so refreshes can update it
    app.set_cache_info(cache_path.clone(), options_hash.clone());

    // Try loading from cache -- if a cache exists, show it immediately and
    // always refresh in the background so the user sees fresh data shortly.
    let cached = if use_cache {
        cache::load_cache(&cache_path, &options_hash)
    } else {
        None
    };

    if let Some(cached_notifications) = cached {
        // Cache hit: display immediately, then refresh in background
        let pinned_notifications =
            state_file::AppStateFile::load_pinned_notifications().unwrap_or_default();
        let mut notifications = cached_notifications;
        for pinned in &pinned_notifications {
            if !notifications.iter().any(|n| n.id == pinned.id) {
                notifications.push(pinned.clone());
            }
        }

        let data = InitialLoadData {
            notifications,
            pinned_notifications,
        };
        app.apply_cached_load(data, settings);

        // Spawn background refresh so we still get fresh data
        let (tx, rx) = channel();
        let config_clone = config.clone();
        let opts_clone = opts.clone();
        let client_clone = client.clone();
        let cache_path_clone = cache_path.clone();
        let options_hash_clone = options_hash.clone();
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
                let extra = fetch_extra_sources(&client_clone, &config_clone, &notifications);
                notifications.extend(extra);
                // Save to cache
                let _ = cache::save_cache(&cache_path_clone, &notifications, &options_hash_clone);

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
        app.start_background_refresh(rx);
    } else {
        // No cache: show loading screen, fetch from API
        let (tx, rx) = channel();
        let config_clone = config.clone();
        let opts_clone = opts.clone();
        let client_clone = client.clone();
        let cache_path_clone = cache_path.clone();
        let options_hash_clone = options_hash.clone();
        let use_cache_clone = use_cache;
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
                let extra = fetch_extra_sources(&client_clone, &config_clone, &notifications);
                notifications.extend(extra);
                // Save to cache if enabled
                if use_cache_clone {
                    let _ =
                        cache::save_cache(&cache_path_clone, &notifications, &options_hash_clone);
                }

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
    }

    // Run the app
    app.run(&mut terminal)?;

    Ok(())
}

fn handle_static_display(config: &Config, opts: &RuntimeOptions) -> Result<()> {
    let client = api::GitHubClient::new(config)?;
    let mut notifications = fetch_notifications(
        &client,
        NotificationFetchOptions {
            show_all: opts.show_all,
            participating: opts.participating,
            max_notifications: opts.max_notifications,
            per_page: config.pagination_size,
        },
    )?;
    let extra = fetch_extra_sources(&client, config, &notifications);
    notifications.extend(extra);

    let filter = Filter::from_config(opts.filter_pattern.as_deref(), config)?;

    for notification in &notifications {
        if !filter.matches(notification) {
            continue;
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
