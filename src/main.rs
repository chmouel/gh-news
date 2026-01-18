mod api;
mod cli;
mod config;
mod error;
mod filter;
mod hooks;
mod markdown;
mod models;
mod preview;
mod state;
mod state_file;
mod terminal;
mod ui;

use clap::Parser;
use cli::Args;
use config::Config;
use error::Result;
use filter::Filter;
use state::AppState;
use terminal::Terminal;
use ui::App;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Merged runtime options from CLI args and config file.
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

    // Handle non-interactive modes first
    if args.mark_read {
        let client = api::GitHubClient::new(&config)?;
        client.mark_all_read(None)?;
        println!("All notifications have been marked as read.");
        return Ok(());
    }

    if args.static_display {
        handle_static_display(&config, &opts)?;
        return Ok(());
    }

    // Interactive mode
    let mut terminal = Terminal::new()?;
    let mut app = App::new(config.clone());

    // Fetch notifications
    let client = api::GitHubClient::new(&config)?;
    let notifications = fetch_notifications(&client, &config, &opts)?;

    // Set API client in app for fetching previews
    app.set_api_client(client);

    // Start background preview worker thread
    app.start_preview_worker();

    // Start auto-refresh if enabled
    app.start_auto_refresh(opts.show_all, opts.participating, opts.max_notifications);

    // Apply filters
    let filter = if opts.filter_pattern.is_some() {
        Some(Filter::new(opts.filter_pattern.as_deref())?)
    } else {
        None
    };

    let mut app_state = AppState::new();
    app_state.set_notifications(notifications);
    app_state.set_filter(filter.clone());
    app_state.set_filter_pattern(opts.filter_pattern.clone());
    app_state.show_all = opts.show_all;

    // Apply repos_collapsed from config
    if config.repos_collapsed {
        app_state.collapse_all_repos();
    }

    // Try to load saved preview mode from state file, fallback to config default
    match state_file::AppStateFile::load() {
        Ok(saved_mode) => {
            app_state.preview_mode = saved_mode;
        }
        Err(_) => {
            app_state.preview_mode = config.get_default_preview_mode();
        }
    }

    app.update_state(app_state);

    // Auto-fetch preview for first notification if preview is enabled
    app.fetch_preview_for_selected();

    // Run the app
    app.run(&mut terminal)?;

    Ok(())
}

fn fetch_notifications(
    client: &api::GitHubClient,
    config: &Config,
    opts: &RuntimeOptions,
) -> Result<Vec<models::Notification>> {
    let max_notifications = opts.max_notifications.unwrap_or(usize::MAX);
    let mut all_notifications = Vec::new();
    let mut page = 1;
    let per_page = config.pagination_size.min(max_notifications);

    loop {
        let notifications = client.get_notifications(
            opts.show_all,
            opts.participating,
            Some(per_page),
            Some(page),
        )?;

        if notifications.is_empty() {
            break;
        }

        let remaining = max_notifications.saturating_sub(all_notifications.len());
        if remaining == 0 {
            break;
        }

        let to_take = remaining.min(notifications.len());
        all_notifications.extend(notifications.into_iter().take(to_take));

        if all_notifications.len() >= max_notifications {
            break;
        }

        page += 1;
    }

    Ok(all_notifications)
}

fn handle_static_display(config: &Config, opts: &RuntimeOptions) -> Result<()> {
    let client = api::GitHubClient::new(config)?;
    let notifications = fetch_notifications(&client, config, opts)?;

    let filter = if opts.filter_pattern.is_some() {
        Some(Filter::new(opts.filter_pattern.as_deref())?)
    } else {
        None
    };

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
