use crate::actions::{self, ActionResult};
use crate::builtin_actions::{self, CombinedAction};
use crate::config::Config;
use crate::error::Result;
use crate::filter::Filter;
use crate::hooks;
use crate::models::Notification;
use crate::models::NotificationType;
use crate::notifications::{fetch_extra_sources, fetch_notifications, NotificationFetchOptions};
use crate::preview::PreviewData;
use crate::preview_manager::{CacheStatus, PreviewManager, PRIORITY_HIGH, PRIORITY_LOW};
use crate::state::{
    AppState, CommandOutputData, ConfirmAction, InputMode, MarkAllOption, PaneFocus, PreviewMode,
};
use crate::state_file::AppStateFile;
use crate::terminal::Terminal;
use crate::ui::components::{
    action_menu, command_output, confirm, filter, help, help_search, list, loading, preview,
    status, url_menu, view_picker,
};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct InitialLoadData {
    pub notifications: Vec<Notification>,
    pub pinned_notifications: Vec<Notification>,
}

#[derive(Debug, Clone)]
pub struct PendingStateSettings {
    pub filter: Option<Filter>,
    pub filter_pattern: Option<String>,
    pub show_all: bool,
    pub repos_collapsed: bool,
    pub preview_mode: PreviewMode,
}

enum BlockingAction {
    Refresh,
    MarkAllRead {
        selected: MarkAllOption,
    },
    ArchiveSelected {
        notification_ids: Vec<String>,
        option: MarkAllOption,
    },
}

/// Pending interactive action to be executed with terminal access.
struct PendingInteractiveAction {
    /// The shell command to execute.
    command: String,
    /// Action name for status message.
    action_name: String,
}

/// Dwell time before auto-marking a notification as read (ms)
const AUTO_MARK_READ_DWELL_MS: u64 = 400;

/// Synthetic notification IDs (from Actions/Events) that have no real
/// GitHub thread and must not be marked read, toggled, or archived.
fn is_synthetic_id(id: &str) -> bool {
    id.starts_with("actions-") || id.starts_with("event-") || id.starts_with("repo-event-")
}

/// Remove previously dismissed synthetic notifications from a list.
fn filter_dismissed_synthetic(notifications: &mut Vec<Notification>) {
    if let Ok(dismissed) = AppStateFile::load_dismissed_synthetic_ids() {
        if !dismissed.is_empty() {
            notifications.retain(|n| !dismissed.contains(&n.id));
        }
    }
}

/// Copy text to the system clipboard via the OSC 52 terminal escape sequence.
fn osc52_copy(text: &str) {
    use base64::Engine;
    use std::io::Write;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(format!("\x1b]52;c;{}\x07", encoded).as_bytes());
    let _ = stdout.flush();
}

pub struct App {
    state: AppState,
    config: Config,
    base_filter: Option<Filter>,
    base_filter_pattern: Option<String>,
    should_quit: bool,
    list_widget: list::ListWidget,
    preview_widget: preview::PreviewWidget,
    status_widget: status::StatusWidget,
    help_widget: help::HelpWidget,
    help_search_widget: help_search::HelpSearchWidget,
    confirm_widget: confirm::ConfirmWidget,
    loading_widget: loading::LoadingWidget,
    filter_widget: filter::FilterWidget,
    action_menu_widget: action_menu::ActionMenuWidget,
    url_menu_widget: url_menu::UrlMenuWidget,
    command_output_widget: command_output::CommandOutputWidget,
    view_picker_widget: view_picker::ViewPickerWidget,
    api_client: Option<crate::api::GitHubClient>,
    last_refresh: Instant,
    refresh_args: Option<(bool, bool, Option<usize>)>, // (all, participating, max_notifications)
    preview_manager: Option<PreviewManager>,
    previous_notification_ids: HashSet<String>, // Track notification IDs for new detection
    initial_load_rx: Option<Receiver<crate::error::Result<InitialLoadData>>>,
    pending_state_settings: Option<PendingStateSettings>,
    pending_blocking_action: Option<BlockingAction>,
    pending_interactive_action: Option<PendingInteractiveAction>,
    pending_print_urls: Vec<String>,
    // Auto-mark-read state
    auto_mark_read_enabled: bool,
    auto_archive_enabled: bool,
    auto_mark_on_open: bool,
    pending_mark_read: Option<(String, Instant)>, // (notification_id, timestamp)
    // Track if pinned notifications need to be saved
    pinned_state_dirty: bool,
    // Notification cache
    cache_path: Option<PathBuf>,
    cache_options_hash: Option<String>,
    background_refresh_rx: Option<Receiver<crate::error::Result<InitialLoadData>>>,
    author_enrichment_rx: Option<Receiver<EnrichmentResult>>,
}

/// Results from the background enrichment thread that resolves author logins
/// and subject state context for notifications.
struct EnrichmentResult {
    authors: std::collections::HashMap<String, String>,
    contexts: std::collections::HashMap<String, String>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let auto_mark_read = config.auto_mark_read;
        let auto_mark_on_open = config.auto_mark_on_open;
        let org_grouping = config.org_grouping;
        let palette = config.color_palette();
        let mut state = AppState::new();
        state.org_grouping = org_grouping;
        Self {
            state,
            config,
            base_filter: None,
            base_filter_pattern: None,
            should_quit: false,
            list_widget: list::ListWidget::new(&palette),
            preview_widget: preview::PreviewWidget::new(&palette),
            status_widget: status::StatusWidget::new(&palette),
            help_widget: help::HelpWidget::new(&palette),
            help_search_widget: help_search::HelpSearchWidget::new(&palette),
            confirm_widget: confirm::ConfirmWidget::new(&palette),
            loading_widget: loading::LoadingWidget::new(&palette),
            filter_widget: filter::FilterWidget::new(&palette),
            action_menu_widget: action_menu::ActionMenuWidget::new(&palette),
            url_menu_widget: url_menu::UrlMenuWidget::new(&palette),
            command_output_widget: command_output::CommandOutputWidget::new(&palette),
            view_picker_widget: view_picker::ViewPickerWidget::new(&palette),
            api_client: None,
            last_refresh: Instant::now(),
            refresh_args: None,
            preview_manager: None,
            previous_notification_ids: HashSet::new(),
            initial_load_rx: None,
            pending_state_settings: None,
            pending_blocking_action: None,
            pending_interactive_action: None,
            pending_print_urls: Vec::new(),
            auto_mark_read_enabled: auto_mark_read,
            auto_archive_enabled: false,
            auto_mark_on_open,
            pending_mark_read: None,
            pinned_state_dirty: false,
            cache_path: None,
            cache_options_hash: None,
            background_refresh_rx: None,
            author_enrichment_rx: None,
        }
    }

    pub fn set_auto_mark_read(&mut self, enabled: bool) {
        self.auto_mark_read_enabled = enabled;
    }

    pub fn set_auto_mark_on_open(&mut self, enabled: bool) {
        self.auto_mark_on_open = enabled;
    }

    pub fn set_auto_archive(&mut self, enabled: bool) {
        self.auto_archive_enabled = enabled;
        // auto_archive implies auto_mark_read
        if enabled {
            self.auto_mark_read_enabled = true;
        }
    }

    /// Queue the currently selected notification for auto-mark-read after dwell time
    fn queue_auto_mark_read(&mut self) {
        if !self.auto_mark_read_enabled {
            return;
        }

        if let Some(notification) = self.state.selected_notification() {
            if notification.is_unread() {
                let notification_id = notification.id.clone();
                self.pending_mark_read = Some((notification_id, Instant::now()));
            } else {
                self.pending_mark_read = None;
            }
        } else {
            self.pending_mark_read = None;
        }
    }

    /// Process pending auto-mark-read if dwell time has elapsed
    fn process_pending_mark_read(&mut self) {
        let dwell_time = Duration::from_millis(AUTO_MARK_READ_DWELL_MS);

        if let Some((ref notification_id, timestamp)) = self.pending_mark_read.clone() {
            if timestamp.elapsed() >= dwell_time {
                // Update local state optimistically
                self.state.mark_notification_read(notification_id);

                if is_synthetic_id(notification_id) {
                    let _ = AppStateFile::dismiss_synthetic_id(notification_id);
                } else if let Some(ref client) = self.api_client {
                    if self.auto_archive_enabled {
                        if let Err(e) = client.mark_thread_done(notification_id) {
                            eprintln!("Failed to auto-archive notification: {}", e);
                        }
                    } else if let Err(e) = client.mark_notification_read(notification_id) {
                        eprintln!("Failed to auto-mark notification as read: {}", e);
                    }
                }

                self.pending_mark_read = None;
            }
        }
    }

    fn save_notifications_cache(&self, notifications: &[Notification]) {
        if let (Some(ref path), Some(ref hash)) = (&self.cache_path, &self.cache_options_hash) {
            let _ = crate::cache::save_cache(path, notifications, hash);
        }
    }

    pub fn set_cache_info(&mut self, path: PathBuf, options_hash: String) {
        self.cache_path = Some(path);
        self.cache_options_hash = Some(options_hash);
    }

    /// Apply cached notification data immediately (no loading screen).
    pub fn apply_cached_load(&mut self, data: InitialLoadData, settings: PendingStateSettings) {
        self.pending_state_settings = Some(settings);
        self.apply_initial_load(data);
    }

    /// Start a background refresh that will silently update notifications
    /// after a cache-hit startup.
    pub fn start_background_refresh(
        &mut self,
        rx: Receiver<crate::error::Result<InitialLoadData>>,
    ) {
        self.background_refresh_rx = Some(rx);
    }

    pub fn set_api_client(&mut self, client: crate::api::GitHubClient) {
        self.preview_manager = Some(PreviewManager::new(client.clone()));
        self.api_client = Some(client);
    }

    pub fn start_initial_load(
        &mut self,
        rx: Receiver<crate::error::Result<InitialLoadData>>,
        settings: PendingStateSettings,
    ) {
        self.initial_load_rx = Some(rx);
        self.pending_state_settings = Some(settings);
        self.state.loading = true;
        self.state.loading_message = "Fetching notifications from GitHub".to_string();
    }

    fn apply_initial_load(&mut self, data: InitialLoadData) {
        let settings = self
            .pending_state_settings
            .take()
            .unwrap_or(PendingStateSettings {
                filter: None,
                filter_pattern: None,
                show_all: false,
                repos_collapsed: self.config.repos_collapsed,
                preview_mode: self.config.get_default_preview_mode(),
            });

        self.base_filter = settings.filter.clone();
        self.base_filter_pattern = settings.filter_pattern.clone();

        let mut notifications = data.notifications;
        filter_dismissed_synthetic(&mut notifications);

        let mut app_state = AppState::new();
        app_state.org_grouping = self.config.org_grouping;
        app_state.views = crate::builtin_views::builtin_views()
            .into_iter()
            .chain(self.config.views.iter().cloned())
            .collect();
        app_state.set_notifications(notifications);
        app_state.set_filter(settings.filter);
        app_state.set_filter_pattern(settings.filter_pattern);
        app_state.show_all = settings.show_all;

        app_state.preview_mode = settings.preview_mode;

        let selected_notif_id = app_state.selected_notification().map(|n| n.id.clone());

        if !data.pinned_notifications.is_empty() {
            app_state.set_pinned_notifications(data.pinned_notifications);
        }

        if settings.repos_collapsed {
            app_state.collapse_all_repos();
        }

        app_state.build_tree();

        if let Some(notification_id) = selected_notif_id {
            if !app_state.select_notification_by_id(&notification_id) {
                app_state.select_first_notification();
            }
        } else {
            app_state.select_first_notification();
        }

        self.update_state(app_state);
        self.fetch_preview_for_selected();
        self.spawn_author_enrichment();

        // Warm the cache for all notifications at low priority so navigation feels instant.
        // The selected notification is already queued at high priority above.
        if let Some(ref pm) = self.preview_manager {
            pm.prefetch_all(&self.state.notifications);
        }

        self.last_refresh = Instant::now();
    }

    fn reapply_filter_preserving_selection(&mut self) {
        let selected_id = self.state.selected_notification().map(|n| n.id.clone());
        self.state.reapply_filter();

        if let Some(selected_id) = selected_id {
            if !self.state.select_notification_by_id(&selected_id) {
                self.state.select_first_notification();
            }
        } else {
            self.state.select_first_notification();
        }

        if self.state.show_preview() {
            self.fetch_preview_for_selected_notification();
        }
    }

    fn build_search_filter(&self) -> Result<Option<Filter>> {
        if self.state.search_query.is_empty() {
            return Ok(None);
        }

        let pattern = &self.state.search_query;
        let filter = Filter::from_pattern(Some(&format!("(?i){}", pattern)))
            .or_else(|_| Filter::from_pattern(Some(&format!("(?i){}", regex::escape(pattern)))))?;
        Ok(Some(filter))
    }

    fn build_active_view_filter(&self) -> Result<Option<Filter>> {
        let Some(view_idx) = self.state.active_view_index else {
            return Ok(self.base_filter.clone());
        };
        let Some(view) = self.state.views.get(view_idx) else {
            return Ok(self.base_filter.clone());
        };

        Ok(Some(Filter::from_view(
            view,
            self.base_filter_pattern.as_deref(),
            &self.config,
        )?))
    }

    fn build_effective_filter(&self) -> Result<Option<Filter>> {
        let filter = self.build_active_view_filter()?;

        if let Some(search_filter) = self.build_search_filter()? {
            Ok(Some(match filter {
                Some(filter) => filter.and(search_filter),
                None => search_filter,
            }))
        } else {
            Ok(filter)
        }
    }

    fn refresh_filter_state(&mut self) -> Result<()> {
        let filter = self.build_effective_filter()?;
        self.state.set_filter(filter);

        let pattern = if self.state.search_query.is_empty() {
            if let Some(view_idx) = self.state.active_view_index {
                self.state
                    .views
                    .get(view_idx)
                    .and_then(|view| view.filter.clone())
            } else {
                self.base_filter_pattern.clone()
            }
        } else {
            Some(self.state.search_query.clone())
        };
        self.state.set_filter_pattern(pattern);
        Ok(())
    }

    /// Open a URL in the browser using custom command if configured, otherwise system default.
    fn open_url_in_browser(&self, url: &str) -> std::io::Result<()> {
        if let Some(ref browser_cmd) = self.config.browser_command {
            if !browser_cmd.is_empty() {
                return std::process::Command::new(browser_cmd)
                    .arg(url)
                    .spawn()
                    .map(|_| ());
            }
        }
        webbrowser::open(url).map_err(std::io::Error::other)
    }

    /// Returns the user-facing verb for the current `open_method`.
    fn open_method_verb(&self) -> &'static str {
        use crate::config::OpenMethod;
        match self.config.open_method {
            OpenMethod::Builtin => "Opened",
            OpenMethod::Osc => "Copied",
            OpenMethod::Print => "Printed",
        }
    }

    /// Deliver a URL according to the configured `open_method`.
    fn deliver_url(&mut self, url: &str) {
        use crate::config::OpenMethod;
        match self.config.open_method {
            OpenMethod::Builtin => {
                if let Err(e) = self.open_url_in_browser(url) {
                    eprintln!("Failed to open URL {}: {}", url, e);
                }
            }
            OpenMethod::Osc => {
                osc52_copy(url);
                self.state.status_message = Some("Copied URL to clipboard".to_string());
            }
            OpenMethod::Print => {
                self.pending_print_urls.push(url.to_string());
            }
        }
    }

    /// Open notification URL using the configured method.
    /// For Discussion notifications, prefer the URL from the cached preview data
    /// (fetched via GraphQL) since `web_url()` may not resolve optimally.
    fn open_notification_url(&mut self, notification: &Notification) {
        let url = self
            .discussion_url_from_preview(notification)
            .or_else(|| notification.web_url(&self.config.github_host));

        if let Some(url) = url {
            self.deliver_url(&url);
        } else {
            eprintln!("No URL available for this notification");
        }
    }

    /// If the notification is a Discussion and we have a cached preview,
    /// return the web URL from that preview data.
    fn discussion_url_from_preview(&self, notification: &Notification) -> Option<String> {
        if !matches!(
            notification.notification_type(),
            NotificationType::Discussion
        ) {
            return None;
        }
        if let Some(preview_manager) = &self.preview_manager {
            if let Some(PreviewData::Discussion { url, .. }) =
                preview_manager.get_cached(&notification.id)
            {
                if !url.is_empty() {
                    return Some(url.clone());
                }
            }
        }
        None
    }

    pub fn start_auto_refresh(
        &mut self,
        all: bool,
        participating: bool,
        max_notifications: Option<usize>,
    ) {
        // Always store refresh args for manual refresh (Ctrl+R)
        self.refresh_args = Some((all, participating, max_notifications));

        // Auto-refresh disabled if interval is 0, but manual refresh still works
        // Store refresh args - we'll check the timer in the main loop
        // No need for a separate thread since we're already polling events
    }

    /// Spawn a background thread to fetch notifications without blocking the UI.
    fn spawn_background_refresh(&mut self) {
        let Some(ref client) = self.api_client else {
            return;
        };
        let Some((all, participating, max_notifications)) = self.refresh_args else {
            return;
        };

        let client = client.clone();
        let config = self.config.clone();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = (|| -> crate::error::Result<InitialLoadData> {
                let mut notifications = fetch_notifications(
                    &client,
                    NotificationFetchOptions {
                        show_all: all,
                        participating,
                        max_notifications,
                        per_page: config.pagination_size,
                    },
                )?;
                let extra = fetch_extra_sources(&client, &config, &notifications);
                notifications.extend(extra);
                Ok(InitialLoadData {
                    notifications,
                    pinned_notifications: Vec::new(),
                })
            })();
            let _ = tx.send(result);
        });

        self.background_refresh_rx = Some(rx);
        self.last_refresh = Instant::now();
    }

    /// Spawn a background thread that resolves author logins and subject state
    /// context for notifications. Results are merged back on the next tick.
    fn spawn_author_enrichment(&mut self) {
        use crate::models::NotificationReason;

        self.author_enrichment_rx = None;

        let Some(client) = self.api_client.clone() else {
            return;
        };

        // Load persisted caches and seed known values into notifications.
        let cached_authors = crate::state_file::load_author_cache();
        let cached_contexts = crate::state_file::load_context_cache();
        for notif in &mut self.state.notifications {
            if notif.author.is_none() {
                if let Some(author) = cached_authors.get(&notif.id) {
                    notif.author = Some(author.clone());
                }
            }
            if notif.context.is_none() {
                if let Some(ctx) = cached_contexts.get(&notif.id) {
                    notif.context = Some(ctx.clone());
                }
            }
        }
        // Re-apply filter so cached values take effect immediately.
        self.reapply_filter_preserving_selection();

        // Each item: (id, comment_url_if_needed, subject_url_if_state_change, is_pr)
        struct FetchItem {
            id: String,
            comment_url: Option<String>,
            subject_url: Option<String>,
            is_pr: bool,
        }

        let to_fetch: Vec<FetchItem> = self
            .state
            .notifications
            .iter()
            .filter_map(|n| {
                let needs_author = n.author.is_none() && n.latest_comment_url.is_some();
                let needs_context =
                    n.reason_enum() == NotificationReason::StateChange && n.subject_url().is_some();

                if !needs_author && !needs_context {
                    return None;
                }

                let is_pr = n.notification_type() == NotificationType::PullRequest;

                Some(FetchItem {
                    id: n.id.clone(),
                    comment_url: if needs_author {
                        n.latest_comment_url.clone()
                    } else {
                        None
                    },
                    subject_url: if needs_context {
                        n.subject_url().map(String::from)
                    } else {
                        None
                    },
                    is_pr,
                })
            })
            .collect();

        if to_fetch.is_empty() {
            return;
        }

        let (tx, rx) = mpsc::channel::<EnrichmentResult>();

        std::thread::spawn(move || {
            // (id, "author"|"context", value)
            let (inner_tx, inner_rx) = mpsc::channel::<(String, &'static str, String)>();

            for item in to_fetch {
                let client = client.clone();
                let inner_tx = inner_tx.clone();
                std::thread::spawn(move || {
                    // Fetch author from latest_comment_url
                    if let Some(url) = &item.comment_url {
                        if let Ok(Some(author)) = client.get_comment_author(url) {
                            let _ = inner_tx.send((item.id.clone(), "author", author));
                        }
                    }

                    // Fetch subject state for state_change notifications
                    if let Some(url) = &item.subject_url {
                        if let Ok(value) = client.get_json_by_url(url) {
                            let context = if item.is_pr {
                                let merged = value
                                    .get("merged")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                if merged {
                                    // Also extract merged_by as author fallback
                                    if item.comment_url.is_none() {
                                        if let Some(login) = value
                                            .get("merged_by")
                                            .and_then(|u| u.get("login"))
                                            .and_then(|l| l.as_str())
                                        {
                                            let _ = inner_tx.send((
                                                item.id.clone(),
                                                "author",
                                                login.to_string(),
                                            ));
                                        }
                                    }
                                    "merged".to_string()
                                } else {
                                    value
                                        .get("state")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("open")
                                        .to_lowercase()
                                }
                            } else {
                                let state = value
                                    .get("state")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("open")
                                    .to_lowercase();
                                if state == "closed" {
                                    if let Some(reason) =
                                        value.get("state_reason").and_then(|v| v.as_str())
                                    {
                                        format!("closed:{reason}")
                                    } else {
                                        state
                                    }
                                } else {
                                    state
                                }
                            };
                            let _ = inner_tx.send((item.id.clone(), "context", context));
                        }
                    }
                });
            }
            drop(inner_tx);

            let mut authors = std::collections::HashMap::new();
            let mut contexts = std::collections::HashMap::new();
            for (id, kind, value) in inner_rx {
                match kind {
                    "author" => {
                        authors.insert(id, value);
                    }
                    "context" => {
                        contexts.insert(id, value);
                    }
                    _ => {}
                }
            }
            let _ = tx.send(EnrichmentResult { authors, contexts });
        });

        self.author_enrichment_rx = Some(rx);
    }

    fn queue_blocking_action(&mut self, action: BlockingAction, message: &str) {
        self.state.loading = true;
        self.state.loading_message = message.to_string();
        self.pending_blocking_action = Some(action);
    }

    fn perform_blocking_action(
        &mut self,
        action: BlockingAction,
        terminal: &mut Terminal,
    ) -> Result<Option<String>> {
        match action {
            BlockingAction::Refresh => {
                self.refresh_notifications()?;
                Ok(None)
            }
            BlockingAction::MarkAllRead { selected } => {
                // Count non-pinned, non-synthetic filtered notifications
                let to_process: Vec<String> = self
                    .state
                    .filtered_notifications
                    .iter()
                    .filter_map(|&idx| {
                        let notif = &self.state.notifications[idx];
                        if !self.state.is_pinned(&notif.id) {
                            Some(notif.id.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                let total = to_process.len();
                let is_filtered = self.state.filter.is_some();

                // Persist dismissed state for synthetic notifications
                let synthetic_ids: Vec<String> = to_process
                    .iter()
                    .filter(|id| is_synthetic_id(id))
                    .cloned()
                    .collect();
                if !synthetic_ids.is_empty() {
                    let _ = AppStateFile::dismiss_synthetic_ids(&synthetic_ids);
                }

                // Take the client temporarily to avoid borrow conflicts during render
                if let Some(client) = self.api_client.take() {
                    for (i, notification_id) in to_process.iter().enumerate() {
                        // Update progress
                        self.state.loading_progress = Some((i + 1, total));

                        // Re-render every 5 items to show progress
                        if i % 5 == 0 || i == total - 1 {
                            terminal.draw(|frame| self.render(frame))?;
                        }

                        // Make the API call
                        match selected {
                            MarkAllOption::MarkReadAndArchive => {
                                let _ = client.mark_thread_done(notification_id);
                            }
                            MarkAllOption::MarkReadOnly => {
                                let _ = client.mark_notification_read(notification_id);
                            }
                        }
                    }

                    // Clear progress and restore client
                    self.state.loading_progress = None;
                    self.api_client = Some(client);

                    self.refresh_notifications()?;
                }
                let msg = match (selected, is_filtered) {
                    (MarkAllOption::MarkReadAndArchive, true) => {
                        format!("Archived {} filtered notifications", total)
                    }
                    (MarkAllOption::MarkReadAndArchive, false) => {
                        format!("Archived {} notifications", total)
                    }
                    (MarkAllOption::MarkReadOnly, true) => {
                        format!("Marked {} filtered notifications as read", total)
                    }
                    (MarkAllOption::MarkReadOnly, false) => format!("Marked {} as read", total),
                };
                Ok(Some(msg))
            }
            BlockingAction::ArchiveSelected {
                notification_ids,
                option,
            } => {
                // Persist dismissed state for synthetic notifications
                let synthetic_ids: Vec<String> = notification_ids
                    .iter()
                    .filter(|id| is_synthetic_id(id))
                    .cloned()
                    .collect();
                if !synthetic_ids.is_empty() {
                    let _ = AppStateFile::dismiss_synthetic_ids(&synthetic_ids);
                }

                let total = notification_ids.len();
                // Take the client temporarily to avoid borrow conflicts during render
                if let Some(client) = self.api_client.take() {
                    for (i, notification_id) in notification_ids.iter().enumerate() {
                        // Update progress
                        self.state.loading_progress = Some((i + 1, total));

                        // Re-render every 5 items to show progress
                        if i % 5 == 0 || i == total - 1 {
                            terminal.draw(|frame| self.render(frame))?;
                        }

                        // Make the API call
                        let result = match option {
                            MarkAllOption::MarkReadAndArchive => {
                                client.mark_thread_done(notification_id)
                            }
                            MarkAllOption::MarkReadOnly => {
                                client.mark_notification_read(notification_id)
                            }
                        };

                        if let Err(e) = result {
                            eprintln!("Failed to process notification {}: {}", notification_id, e);
                        }
                    }

                    // Clear progress and restore client
                    self.state.loading_progress = None;
                    self.api_client = Some(client);

                    self.refresh_notifications()?;
                }
                let msg = match option {
                    MarkAllOption::MarkReadAndArchive => {
                        format!("Archived {} selected notifications", total)
                    }
                    MarkAllOption::MarkReadOnly => {
                        format!("Marked {} selected notifications as read", total)
                    }
                };
                Ok(Some(msg))
            }
        }
    }

    fn refresh_notifications(&mut self) -> Result<()> {
        if let Some(ref client) = self.api_client {
            if let Some((all, participating, max_notifications)) = self.refresh_args {
                // Loading state is managed by the caller to avoid auto-refresh flicker.

                let mut all_notifications = fetch_notifications(
                    client,
                    NotificationFetchOptions {
                        show_all: all,
                        participating,
                        max_notifications,
                        per_page: self.config.pagination_size,
                    },
                )?;

                // Fetch opt-in extra sources (Actions, Events)
                let extra = fetch_extra_sources(client, &self.config, &all_notifications);
                all_notifications.extend(extra);

                // Filter out previously dismissed synthetic notifications
                filter_dismissed_synthetic(&mut all_notifications);

                // Update notification cache
                self.save_notifications_cache(&all_notifications);

                for pinned in self.state.get_pinned_notifications() {
                    if !all_notifications.iter().any(|n| n.id == pinned.id) {
                        all_notifications.push(pinned.clone());
                    }
                }

                // Preserve current selection if possible
                let old_notif_id = self.state.selected_notification().map(|n| n.id.clone());

                // Preserve current filter
                let current_filter = self.state.filter.clone();
                let filter_pattern = self.state.filter_pattern.clone();

                self.state.set_notifications(all_notifications);

                // Mark stale cached previews and get the set of invalidated IDs.
                let invalidated = if let Some(ref pm) = self.preview_manager {
                    pm.invalidate_notifications(&self.state.notifications)
                } else {
                    std::collections::HashSet::new()
                };

                // Execute hook for new notifications if configured
                if let Some(ref hook_command) = self.config.on_new_notification_command {
                    if !hook_command.is_empty() {
                        // Build set of current notification IDs
                        let current_ids: HashSet<String> = self
                            .state
                            .notifications
                            .iter()
                            .map(|n| n.id.clone())
                            .collect();

                        // Only execute hooks if we have a baseline (not first load)
                        if !self.previous_notification_ids.is_empty() {
                            // Find IDs that are new (in current but not in previous)
                            for new_id in current_ids.difference(&self.previous_notification_ids) {
                                if let Some(notification) =
                                    self.state.notifications.iter().find(|n| &n.id == new_id)
                                {
                                    if let Err(e) = hooks::execute_new_notification_hook(
                                        hook_command,
                                        notification,
                                        &self.config.github_host,
                                    ) {
                                        eprintln!(
                                            "Failed to execute notification hook for '{}': {}",
                                            notification.title(),
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // Update previous IDs for next refresh cycle
                        self.previous_notification_ids = current_ids;
                    }
                }

                // Always update previous IDs if not done above (no hook configured)
                if self.previous_notification_ids.is_empty() && !self.state.notifications.is_empty()
                {
                    self.previous_notification_ids = self
                        .state
                        .notifications
                        .iter()
                        .map(|n| n.id.clone())
                        .collect();
                }

                // Restore filter
                self.state.set_filter(current_filter);
                self.state.set_filter_pattern(filter_pattern);

                // Try to restore selection by ID
                if let Some(old_id) = old_notif_id {
                    if !self.state.select_notification_by_id(&old_id) {
                        self.state.select_first_notification();
                    }
                } else {
                    self.state.select_first_notification();
                }

                // Update the preview pane for the restored selection.
                let selected_was_invalidated = self
                    .state
                    .selected_notification()
                    .map(|n| invalidated.contains(&n.id))
                    .unwrap_or(false);

                if self.state.show_preview() {
                    // Always refresh preview content for the current selection so stale
                    // content from a previous selection is never shown.
                    self.fetch_preview_for_selected_notification();
                } else if selected_was_invalidated {
                    // Preview pane is off but the selected entry was invalidated: clear the
                    // cached UI content so pressing Tab won't show stale data.
                    self.state.preview_content = None;
                }

                // Background-revalidate all remaining stale entries (low priority).
                // The selected notification is already handled above at high priority.
                if let Some(ref pm) = self.preview_manager {
                    let skip_id = self.state.selected_notification().map(|n| n.id.clone());
                    pm.revalidate_all_stale(&self.state.notifications, skip_id.as_deref());
                }

                // Prefetch neighbours regardless of preview visibility.
                self.prefetch_neighbour_previews();
                self.spawn_author_enrichment();

                self.state.loading = false;
                self.last_refresh = Instant::now();
            }
        }
        Ok(())
    }

    /// Merge freshly fetched notifications into the current state, preserving
    /// the user's selection, filter, scroll position, and collapsed repos.
    /// Used by background refresh (cache-hit startup and auto-refresh timer).
    fn merge_refreshed_notifications(&mut self, mut notifications: Vec<Notification>) {
        // Filter out previously dismissed synthetic notifications
        filter_dismissed_synthetic(&mut notifications);

        // Save the notification cache
        self.save_notifications_cache(&notifications);

        // Merge pinned notifications
        for pinned in self.state.get_pinned_notifications() {
            if !notifications.iter().any(|n| n.id == pinned.id) {
                notifications.push(pinned.clone());
            }
        }

        // Preserve current selection
        let old_notif_id = self.state.selected_notification().map(|n| n.id.clone());

        // Preserve current filter
        let current_filter = self.state.filter.clone();
        let filter_pattern = self.state.filter_pattern.clone();

        let old_count = self.state.notifications.len();
        self.state.set_notifications(notifications);

        // Mark stale cached previews
        if let Some(ref pm) = self.preview_manager {
            pm.invalidate_notifications(&self.state.notifications);
        }

        // Execute hook for new notifications if configured
        let current_ids: HashSet<String> = self
            .state
            .notifications
            .iter()
            .map(|n| n.id.clone())
            .collect();

        if let Some(ref hook_command) = self.config.on_new_notification_command.clone() {
            if !hook_command.is_empty() && !self.previous_notification_ids.is_empty() {
                for new_id in current_ids.difference(&self.previous_notification_ids) {
                    if let Some(notification) =
                        self.state.notifications.iter().find(|n| &n.id == new_id)
                    {
                        let _ = hooks::execute_new_notification_hook(
                            hook_command,
                            notification,
                            &self.config.github_host,
                        );
                    }
                }
            }
        }

        // Count new notifications for status message
        let new_count = if !self.previous_notification_ids.is_empty() {
            current_ids
                .difference(&self.previous_notification_ids)
                .count()
        } else {
            0
        };

        // Update previous notification IDs for hook tracking
        self.previous_notification_ids = current_ids;

        // Show status message if new notifications arrived
        if new_count > 0 {
            self.state.status_message = Some(format!("{} new", new_count));
        } else if old_count > 0 {
            self.state.status_message = Some("Refreshed".to_string());
        }

        // Restore filter
        self.state.set_filter(current_filter);
        self.state.set_filter_pattern(filter_pattern);

        // Restore selection
        if let Some(old_id) = old_notif_id {
            if !self.state.select_notification_by_id(&old_id) {
                self.state.select_first_notification();
            }
        } else {
            self.state.select_first_notification();
        }

        // Refresh preview for the current selection
        if self.state.show_preview() {
            self.fetch_preview_for_selected_notification();
        }

        self.spawn_author_enrichment();
        self.prefetch_neighbour_previews();
        self.last_refresh = Instant::now();
    }

    pub fn update_state(&mut self, state: AppState) {
        self.state = state;
        // Auto-fetch preview for first notification if preview is enabled
        self.auto_fetch_preview_for_selected();
    }

    fn auto_fetch_preview_for_selected(&mut self) {
        // Only auto-fetch if preview is enabled and there's no content yet.
        // Used for initial load.
        if !self.state.show_preview() || self.state.preview_content.is_some() {
            return;
        }

        self.fetch_preview_for_selected_notification();
        self.prefetch_neighbour_previews();
    }

    fn fetch_preview_for_selected_notification(&mut self) {
        let notification = match self.state.selected_notification() {
            Some(n) => n.clone(),
            None => {
                self.state.preview_content = None;
                return;
            }
        };

        let Some(preview_manager) = self.preview_manager.as_ref() else {
            self.state.preview_content = None;
            return;
        };

        match preview_manager.get_cached_status(&notification) {
            CacheStatus::Fresh(data) => {
                self.state.preview_content = Some(data);
                self.state.preview_scroll = 0;
            }
            CacheStatus::Stale(data) => {
                // Show the cached content immediately and revalidate in the background.
                self.state.preview_content = Some(data);
                self.state.preview_scroll = 0;
                preview_manager.request_revalidation(&notification, PRIORITY_HIGH);
            }
            CacheStatus::Miss => {
                if preview_manager.is_loading(&notification.id) {
                    self.state.preview_content = Some(PreviewData::Generic {
                        title: "Loading details...".to_string(),
                        body: "⏳ Fetching details...\n\nThis may take a moment.".to_string(),
                    });
                    return;
                }
                preview_manager.request_preview(&notification, PRIORITY_HIGH);
                self.state.preview_content = Some(PreviewData::Generic {
                    title: "Loading details...".to_string(),
                    body: "⏳ Fetching details...\n\nThis may take a moment.".to_string(),
                });
                self.state.preview_scroll = 0;
            }
        }
    }

    pub fn fetch_preview_for_selected(&mut self) {
        self.auto_fetch_preview_for_selected();
    }

    fn is_preview_showing_loading_placeholder(&self) -> bool {
        matches!(
            &self.state.preview_content,
            Some(PreviewData::Generic { title, .. }) if title == "Loading details..."
        )
    }

    /// Prefetch the notification immediately before and after the current selection.
    ///
    /// Runs regardless of whether the preview pane is currently visible so that opening
    /// the pane feels instant.  Only queues a request if the entry is not already cached
    /// or in-flight.
    fn prefetch_neighbour_previews(&mut self) {
        let Some(preview_manager) = self.preview_manager.as_ref() else {
            return;
        };

        let current_idx = self.state.selected_index;

        // Helper: queue a prefetch for the notification at tree index `i` if needed.
        let find_notif = |i: usize| -> Option<Notification> {
            if let Some(crate::state::TreeItem::Notification(notif_idx)) =
                self.state.tree_items.get(i)
            {
                self.state.notifications.get(*notif_idx).cloned()
            } else {
                None
            }
        };

        // One ahead.
        let next_notif = (current_idx + 1..self.state.tree_items.len()).find_map(&find_notif);

        // One behind.
        let prev_notif = (0..current_idx).rev().find_map(find_notif);

        for notif in [next_notif, prev_notif].into_iter().flatten() {
            if preview_manager.get_cached(&notif.id).is_none()
                && !preview_manager.is_loading(&notif.id)
            {
                preview_manager.request_preview(&notif, PRIORITY_LOW);
            }
        }
    }

    pub fn run(&mut self, terminal: &mut Terminal) -> Result<()> {
        // Initial draw to show the UI
        terminal.draw(|frame| {
            self.render(frame);
        })?;

        loop {
            if self.should_quit {
                break;
            }

            if let Some(ref rx) = self.initial_load_rx {
                match rx.try_recv() {
                    Ok(result) => {
                        self.initial_load_rx = None;
                        match result {
                            Ok(data) => self.apply_initial_load(data),
                            Err(e) => return Err(e),
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        return Err(crate::error::Error::Terminal(
                            "Initial load channel closed unexpectedly".to_string(),
                        ));
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }

            // Poll enrichment results (authors + contexts)
            if let Some(ref rx) = self.author_enrichment_rx {
                match rx.try_recv() {
                    Ok(result) => {
                        self.author_enrichment_rx = None;
                        for notif in &mut self.state.notifications {
                            if let Some(author) = result.authors.get(&notif.id) {
                                notif.author = Some(author.clone());
                            }
                            if let Some(ctx) = result.contexts.get(&notif.id) {
                                notif.context = Some(ctx.clone());
                            }
                        }
                        self.reapply_filter_preserving_selection();
                        // Persist resolved authors and contexts for future sessions.
                        let all_authors: std::collections::HashMap<String, String> = self
                            .state
                            .notifications
                            .iter()
                            .filter_map(|n| n.author.as_ref().map(|a| (n.id.clone(), a.clone())))
                            .collect();
                        let _ = crate::state_file::save_author_cache(&all_authors);
                        let all_contexts: std::collections::HashMap<String, String> = self
                            .state
                            .notifications
                            .iter()
                            .filter_map(|n| n.context.as_ref().map(|c| (n.id.clone(), c.clone())))
                            .collect();
                        let _ = crate::state_file::save_context_cache(&all_contexts);
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.author_enrichment_rx = None;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }

            // Poll background refresh (after cache-hit startup)
            if let Some(ref rx) = self.background_refresh_rx {
                match rx.try_recv() {
                    Ok(Ok(data)) => {
                        self.background_refresh_rx = None;
                        self.merge_refreshed_notifications(data.notifications);
                    }
                    Ok(Err(_)) => {
                        // Background refresh failed; keep showing cached data
                        self.background_refresh_rx = None;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.background_refresh_rx = None;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }

            if self.initial_load_rx.is_none() {
                if let Some(action) = self.pending_blocking_action.take() {
                    let status_message = self.perform_blocking_action(action, terminal)?;
                    self.state.loading = false;
                    self.state.loading_message.clear();
                    self.state.loading_progress = None;
                    if status_message.is_some() {
                        self.state.status_message = status_message;
                    }
                }
            }

            // Handle pending interactive action (requires terminal suspend/resume)
            if let Some(interactive_action) = self.pending_interactive_action.take() {
                // Suspend TUI for interactive command
                terminal.suspend()?;

                // Execute the command with full terminal access
                let result = actions::execute_interactive(&interactive_action.command);

                // Resume TUI
                terminal.resume()?;

                // Set status message based on result
                self.state.status_message = Some(match result {
                    Ok(true) => format!("{}: done", interactive_action.action_name),
                    Ok(false) => format!("{}: exited with error", interactive_action.action_name),
                    Err(e) => format!("{}: {}", interactive_action.action_name, e),
                });
            }

            // Handle pending print URLs (suspend TUI, print, wait for keypress)
            if !self.pending_print_urls.is_empty() {
                let urls = std::mem::take(&mut self.pending_print_urls);
                terminal.suspend()?;

                for url in &urls {
                    println!("{url}");
                }
                println!("\nPress Enter to return...");
                let _ = std::io::stdin().read_line(&mut String::new());

                terminal.resume()?;
            }

            // Check for auto-refresh signal (non-blocking background fetch)
            if self.config.auto_refresh_interval > 0
                && !self.state.loading
                && self.background_refresh_rx.is_none()
            {
                let elapsed = self.last_refresh.elapsed();
                if elapsed >= Duration::from_secs(self.config.auto_refresh_interval) {
                    self.spawn_background_refresh();
                }
            }

            // Poll for background fetch completions (non-blocking)
            if let Some(ref preview_manager) = self.preview_manager {
                for completed_id in preview_manager.drain_completed() {
                    if let Some(current_notif) = self.state.selected_notification() {
                        if current_notif.id == completed_id {
                            if let Some(data) = preview_manager.get_cached(&completed_id) {
                                self.state.preview_content = Some(data);
                                self.state.preview_scroll = 0;
                            }
                        }
                    }
                }
            }

            // Safety net: if the preview is stuck on the loading placeholder,
            // re-check cache status for the currently selected notification.
            if self.is_preview_showing_loading_placeholder() {
                self.fetch_preview_for_selected_notification();
            }

            // Process pending auto-mark-read (debounced)
            self.process_pending_mark_read();

            // Use standard timeout for native ratatui behavior
            let timeout = Duration::from_millis(250); // Standard ratatui polling timeout

            if event::poll(timeout).map_err(|e| crate::error::Error::Terminal(e.to_string()))? {
                match event::read().map_err(|e| crate::error::Error::Terminal(e.to_string()))? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            if self.state.loading {
                                if key.code == KeyCode::Char('q')
                                    || key.code == KeyCode::Esc
                                    || (key.code == KeyCode::Char('c')
                                        && key.modifiers.contains(KeyModifiers::CONTROL))
                                {
                                    self.should_quit = true;
                                }
                            } else {
                                self.handle_key(key)?;
                            }
                        }
                    }
                    Event::Resize(_, _) => {
                        // Terminal was resized - ratatui will handle redraw
                    }
                    Event::Mouse(mouse_event) => {
                        // Only handle mouse events when not in help or loading
                        if !self.state.show_help && !self.state.loading {
                            // Get terminal size for layout calculations
                            let size = terminal.size()?;
                            self.handle_mouse(mouse_event, size)?;
                        }
                    }
                    _ => {}
                }
            }

            // Always redraw after event loop - native ratatui pattern
            terminal.draw(|frame| {
                self.render(frame);
            })?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.size();

        // Ensure minimum terminal size
        if size.width < 40 || size.height < 10 {
            let error_msg = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Terminal too small!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from("Please resize your terminal to at least 40x10"),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Current size: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{}x{}", size.width, size.height),
                        Style::default().fg(Color::White),
                    ),
                ]),
            ];

            let paragraph = Paragraph::new(error_msg)
                .block(Block::default().borders(Borders::ALL).title(" Error "))
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, size);
            return;
        }

        if self.state.show_help {
            let layout = help::HelpWidget::layout(size);
            let filter = self.state.help_search_query.trim();
            let filter = if filter.is_empty() {
                None
            } else {
                Some(filter)
            };
            let content = self.help_widget.build_content(filter);
            self.state.help_view_height = layout.inner_height;
            self.state.help_content_len =
                help::HelpWidget::content_height(&content, layout.inner_width);
            self.clamp_help_scroll();
            self.help_widget
                .render(frame, layout, &content, self.state.help_scroll);
            if self.state.input_mode == InputMode::HelpSearch
                || !self.state.help_search_query.is_empty()
            {
                self.help_search_widget.render(
                    frame,
                    size,
                    &self.state.help_search_query,
                    content.match_count,
                    content.total_lines,
                    self.state.input_mode == InputMode::HelpSearch,
                );
            }
            return;
        }

        if self.state.loading {
            self.loading_widget.render(
                frame,
                size,
                &self.state.loading_message,
                self.state.loading_progress,
            );
            if self.state.input_mode == InputMode::ViewPicker {
                self.view_picker_widget.render(
                    frame,
                    size,
                    &self.state.views.clone(),
                    self.state.view_picker_index,
                    self.state.active_view_index,
                );
            }
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(0),    // Main content
            ])
            .split(size);

        let refresh_state = status::RefreshState {
            last_refresh: self.last_refresh,
            is_refreshing: self.background_refresh_rx.is_some(),
        };
        self.status_widget.render(
            frame,
            chunks[0],
            &self.state,
            &self.config,
            self.auto_mark_read_enabled,
            &refresh_state,
        );

        let preview_mode = self.effective_preview_mode();

        // Check if a pane is focused (zoomed)
        match self.state.focused_pane {
            PaneFocus::Pane1 => {
                // Show only list widget in full screen
                self.list_widget
                    .render(frame, chunks[1], &self.state, &self.config);
            }
            PaneFocus::Pane2 => {
                // Show only preview widget in full screen (if preview is enabled)
                if self.state.show_preview() {
                    self.preview_widget.render(frame, chunks[1], &self.state);
                } else {
                    // If preview is off but pane 2 is focused, show list instead
                    self.list_widget
                        .render(frame, chunks[1], &self.state, &self.config);
                }
            }
            PaneFocus::None => {
                // Normal split layout based on preview_mode
                let main_chunks = match preview_mode {
                    PreviewMode::Off => Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(100)])
                        .split(chunks[1]),
                    PreviewMode::Horizontal => {
                        Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([
                                Constraint::Ratio(1, 3), // List: 1/3 of width
                                Constraint::Ratio(2, 3), // Preview: 2/3 of width
                            ])
                            .split(chunks[1])
                    }
                    PreviewMode::Vertical => {
                        Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Ratio(1, 3), // List: 1/3 of height
                                Constraint::Ratio(2, 3), // Preview: 2/3 of height
                            ])
                            .split(chunks[1])
                    }
                };

                match preview_mode {
                    PreviewMode::Off => {
                        self.list_widget
                            .render(frame, main_chunks[0], &self.state, &self.config);
                    }
                    PreviewMode::Horizontal => {
                        self.list_widget
                            .render(frame, main_chunks[0], &self.state, &self.config);
                        self.preview_widget
                            .render(frame, main_chunks[1], &self.state);
                    }
                    PreviewMode::Vertical => {
                        self.list_widget
                            .render(frame, main_chunks[0], &self.state, &self.config);
                        self.preview_widget
                            .render(frame, main_chunks[1], &self.state);
                    }
                }
            }
        }

        // Render filter input as overlay (if in search mode)
        if self.state.input_mode == InputMode::Search {
            self.filter_widget.render(frame, size, &self.state);
        }

        // Render confirmation dialog as overlay (if active)
        if self.state.input_mode == InputMode::Confirm {
            if let Some(ref action) = self.state.confirm_action {
                let count = match action {
                    ConfirmAction::ArchiveSelected { count, .. } => *count,
                    ConfirmAction::MarkAllRead { .. } => self
                        .state
                        .filtered_notifications
                        .iter()
                        .filter(|&&idx| !self.state.is_pinned(&self.state.notifications[idx].id))
                        .count(),
                };
                let is_filtered = self.state.filter.is_some();
                self.confirm_widget
                    .render(frame, size, action, count, is_filtered);
            }
        }

        // Render action menu as overlay (if active)
        if self.state.input_mode == InputMode::ActionMenu {
            let notification_count = if self.state.has_selection() {
                self.state.selection_count()
            } else {
                1
            };
            let all_actions = builtin_actions::get_all_actions(&self.config.actions);
            self.action_menu_widget.render(
                frame,
                size,
                &all_actions,
                self.state.action_menu_index,
                notification_count,
            );
        }

        // Render URL menu as overlay (if active)
        if self.state.input_mode == InputMode::UrlMenu {
            self.url_menu_widget
                .render(frame, size, self.state.url_menu_index);
        }

        // Render command output popup as overlay (if active)
        if self.state.input_mode == InputMode::CommandOutput {
            if let Some(ref out) = self.state.command_output {
                self.command_output_widget.render(frame, size, out);
            }
        }

        // Render view picker popup as overlay (if active)
        if self.state.input_mode == InputMode::ViewPicker {
            self.view_picker_widget.render(
                frame,
                size,
                &self.state.views.clone(),
                self.state.view_picker_index,
                self.state.active_view_index,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.state.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Search => self.handle_search_key(key),
            InputMode::Help => self.handle_help_key(key),
            InputMode::HelpSearch => self.handle_help_search_key(key),
            InputMode::Confirm => self.handle_confirm_key(key),
            InputMode::ActionMenu => self.handle_action_menu_key(key),
            InputMode::UrlMenu => self.handle_url_menu_key(key),
            InputMode::CommandOutput => self.handle_command_output_key(key),
            InputMode::ViewPicker => self.handle_view_picker_key(key),
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
                self.state.confirm_action = None;
            }
            KeyCode::Up | KeyCode::Char('k') => match &mut self.state.confirm_action {
                Some(ConfirmAction::MarkAllRead { ref mut selected }) => {
                    *selected = MarkAllOption::MarkReadAndArchive;
                }
                Some(ConfirmAction::ArchiveSelected { ref mut option, .. }) => {
                    *option = MarkAllOption::MarkReadAndArchive;
                }
                None => {}
            },
            KeyCode::Down | KeyCode::Char('j') => match &mut self.state.confirm_action {
                Some(ConfirmAction::MarkAllRead { ref mut selected }) => {
                    *selected = MarkAllOption::MarkReadOnly;
                }
                Some(ConfirmAction::ArchiveSelected { ref mut option, .. }) => {
                    *option = MarkAllOption::MarkReadOnly;
                }
                None => {}
            },
            KeyCode::Enter => {
                if let Some(action) = self.state.confirm_action.take() {
                    self.execute_confirmed_action(action)?;
                }
                self.state.input_mode = InputMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action_menu_key(&mut self, key: KeyEvent) -> Result<()> {
        let all_actions = builtin_actions::get_all_actions(&self.config.actions);
        let action_count = all_actions.len();
        if action_count == 0 {
            self.state.input_mode = InputMode::Normal;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.action_menu_index > 0 {
                    self.state.action_menu_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.action_menu_index < action_count - 1 {
                    self.state.action_menu_index += 1;
                }
            }
            KeyCode::Enter => {
                self.execute_selected_action();
                // Don't override input_mode if execute_selected_action set a specific mode
                // (e.g. CommandOutput). Only reset to Normal when still in ActionMenu.
                if self.state.input_mode == InputMode::ActionMenu {
                    self.state.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Char(c) => {
                if let Some(index) = builtin_actions::index_for_shortcut(c) {
                    if index < action_count {
                        self.state.action_menu_index = index;
                        self.execute_selected_action();
                        if self.state.input_mode == InputMode::ActionMenu {
                            self.state.input_mode = InputMode::Normal;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_url_menu_key(&mut self, key: KeyEvent) -> Result<()> {
        let item_count = url_menu::URL_MENU_ITEMS.len();
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.url_menu_index > 0 {
                    self.state.url_menu_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.url_menu_index < item_count - 1 {
                    self.state.url_menu_index += 1;
                }
            }
            KeyCode::Enter => {
                self.execute_url_menu_action(self.state.url_menu_index);
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c @ '1'..='3') => {
                let index = (c as usize) - ('1' as usize);
                if index < item_count {
                    self.execute_url_menu_action(index);
                    self.state.input_mode = InputMode::Normal;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_url_menu_action(&mut self, index: usize) {
        // Collect URLs first to avoid borrow issues
        let urls: Vec<String> = if self.state.has_selection() {
            let selected_ids = self.state.get_selected_notification_ids();
            selected_ids
                .iter()
                .filter_map(|id| {
                    self.state
                        .notifications
                        .iter()
                        .find(|n| &n.id == id)
                        .and_then(|n| {
                            self.discussion_url_from_preview(n)
                                .or_else(|| n.web_url(&self.config.github_host))
                        })
                })
                .collect()
        } else if let Some(notification) = self.state.selected_notification() {
            self.discussion_url_from_preview(notification)
                .or_else(|| notification.web_url(&self.config.github_host))
                .into_iter()
                .collect()
        } else {
            return;
        };

        if urls.is_empty() {
            return;
        }

        match index {
            0 => {
                // Open in browser
                for url in &urls {
                    if let Err(e) = self.open_url_in_browser(url) {
                        eprintln!("Failed to open URL {}: {}", url, e);
                    }
                }
            }
            1 => {
                // Copy via OSC 52
                for url in &urls {
                    osc52_copy(url);
                }
                self.state.status_message = Some("Copied URL to clipboard".to_string());
            }
            2 => {
                // Print URL
                self.pending_print_urls.extend(urls);
            }
            _ => {}
        }

        if self.state.has_selection() {
            self.state.clear_selection();
        }
    }

    fn handle_view_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        let total_items = self.state.views.len() + 1;

        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.view_picker_index > 0 {
                    self.state.view_picker_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.view_picker_index + 1 < total_items {
                    self.state.view_picker_index += 1;
                }
            }
            KeyCode::Enter => {
                let index = self.state.view_picker_index;
                self.apply_view(index)?;
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = (c as usize).saturating_sub('0' as usize);
                if digit < total_items {
                    self.apply_view(digit)?;
                    self.state.input_mode = InputMode::Normal;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Apply a named view by picker index (0 = default, 1+ = views[index-1]).
    fn apply_view(&mut self, index: usize) -> Result<()> {
        if index == 0 {
            self.state.active_view_index = None;
            self.refresh_filter_state()?;
        } else {
            let view_idx = index - 1;
            if let Some(view) = self.state.views.get(view_idx).cloned() {
                match Filter::from_view(&view, self.base_filter_pattern.as_deref(), &self.config) {
                    Ok(_) => {
                        self.state.active_view_index = Some(view_idx);
                        self.refresh_filter_state()?;
                    }
                    Err(err) => {
                        self.state.status_message =
                            Some(format!("View '{}' is invalid: {}", view.name, err));
                    }
                }
            }
        }
        self.state.view_picker_index = 0;
        Ok(())
    }

    fn handle_command_output_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.command_output = None;
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut out) = self.state.command_output {
                    out.scroll = out.scroll.saturating_add(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut out) = self.state.command_output {
                    out.scroll = out.scroll.saturating_sub(1);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut out) = self.state.command_output {
                    out.scroll = out.scroll.saturating_add(10);
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut out) = self.state.command_output {
                    out.scroll = out.scroll.saturating_sub(10);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn capture_show_output_command(command: &str) -> std::result::Result<String, String> {
        if command.trim().is_empty() {
            return Err("Empty command".to_string());
        }

        let output = actions::execute_and_capture(command)?;
        let output = output.trim_end();

        if output.trim().is_empty() {
            Ok("(no output)".to_string())
        } else {
            Ok(output.to_string())
        }
    }

    fn capture_show_output_action(
        action: &crate::config::Action,
        notifications: &[Notification],
        github_host: &str,
    ) -> std::result::Result<String, String> {
        if actions::has_batch_placeholders(&action.command) {
            let command = actions::prepare_batch_command(action, notifications, github_host);
            return Self::capture_show_output_command(&command);
        }

        let show_headers = notifications.len() > 1;
        let mut outputs = Vec::with_capacity(notifications.len());

        for notification in notifications {
            let command = actions::prepare_command(action, notification, github_host);
            let output = Self::capture_show_output_command(&command)?;

            if show_headers {
                outputs.push(format!(
                    "{} ({})\n{}",
                    notification.title(),
                    notification.id,
                    output
                ));
            } else {
                outputs.push(output);
            }
        }

        Ok(outputs.join("\n\n"))
    }

    fn execute_selected_action(&mut self) {
        let action_index = self.state.action_menu_index;
        let all_actions = builtin_actions::get_all_actions(&self.config.actions);
        let action = match all_actions.get(action_index) {
            Some(a) => a.clone(),
            None => return,
        };

        // Collect notifications to run action on
        let notifications: Vec<Notification> = if self.state.has_selection() {
            let selected_ids = self.state.get_selected_notification_ids();
            self.state
                .notifications
                .iter()
                .filter(|n| selected_ids.contains(&n.id))
                .cloned()
                .collect()
        } else if let Some(notif) = self.state.selected_notification() {
            vec![notif.clone()]
        } else {
            return;
        };

        if notifications.is_empty() {
            return;
        }

        // Handle built-in actions
        if let CombinedAction::Builtin(builtin) = &action {
            let msg = if let Some(ref client) = self.api_client {
                if notifications.len() > 1 {
                    match builtin.execute_batch(&notifications, client) {
                        Ok(msg) => msg,
                        Err(e) => format!("{}: {}", builtin.name(), e),
                    }
                } else {
                    match builtin.execute(&notifications[0], client) {
                        Ok(msg) => msg,
                        Err(e) => format!("{}: {}", builtin.name(), e),
                    }
                }
            } else {
                format!("{}: API client not available", builtin.name())
            };

            // Clear selection after executing action
            if self.state.has_selection() {
                self.state.clear_selection();
            }

            self.state.status_message = Some(msg);
            self.state.rebuild_after_changes();
            self.fetch_preview_for_selected_notification();
            self.prefetch_neighbour_previews();
            return;
        }

        // Handle custom actions
        let CombinedAction::Custom(custom_action) = action else {
            return;
        };

        let count = notifications.len();
        let github_host = self.config.github_host.clone();
        let action_name = custom_action.name.clone();
        let is_batch = actions::has_batch_placeholders(&custom_action.command);

        // Interactive actions: prepare command and defer execution to main loop
        // (requires terminal access for suspend/resume)
        if custom_action.interactive {
            let command = if is_batch {
                // Batch mode: run single command with all notifications
                actions::prepare_batch_command(&custom_action, &notifications, &github_host)
            } else {
                // Non-batch: only run on first notification
                notifications
                    .first()
                    .map(|n| actions::prepare_command(&custom_action, n, &github_host))
                    .unwrap_or_default()
            };

            if !command.trim().is_empty() {
                self.pending_interactive_action = Some(PendingInteractiveAction {
                    command,
                    action_name,
                });
            }

            // Clear selection
            if self.state.has_selection() {
                self.state.clear_selection();
            }
            return;
        }

        // show_output actions: run synchronously and display output in a popup
        if custom_action.show_output {
            match Self::capture_show_output_action(&custom_action, &notifications, &github_host) {
                Ok(content) => {
                    self.state.command_output = Some(CommandOutputData {
                        title: action_name,
                        content,
                        scroll: 0,
                    });
                    self.state.input_mode = InputMode::CommandOutput;
                }
                Err(e) => {
                    self.state.command_output = None;
                    self.state.status_message = Some(format!("{}: {}", action_name, e));
                }
            }

            if self.state.has_selection() {
                self.state.clear_selection();
            }
            return;
        }

        // Non-interactive actions: spawn in background
        let msg = if is_batch {
            // Batch mode: single command with all notifications
            match actions::execute_batch_action(&custom_action, &notifications, &github_host) {
                ActionResult::Spawned => format!(
                    "{}: ran on {} notifications",
                    action_name,
                    notifications.len()
                ),
                ActionResult::Failed(error) => format!("{}: {}", action_name, error),
            }
        } else {
            // Non-batch: one command per notification
            let mut success_count = 0;
            let mut last_error = String::new();

            for notification in &notifications {
                match actions::execute_action(&custom_action, notification, &github_host) {
                    ActionResult::Spawned => {
                        success_count += 1;
                    }
                    ActionResult::Failed(error) => {
                        last_error = error;
                    }
                }
            }

            if !last_error.is_empty() {
                format!("{}: {}", action_name, last_error)
            } else if count > 1 {
                format!("{}: ran on {} notifications", action_name, success_count)
            } else {
                format!("{}: done", action_name)
            }
        };

        // Clear selection after executing action
        if self.state.has_selection() {
            self.state.clear_selection();
        }

        self.state.status_message = Some(msg);
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                self.close_help();
            }
            KeyCode::Char('/') => {
                self.state.input_mode = InputMode::HelpSearch;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_help_by(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_help_by(1);
            }
            KeyCode::PageUp => {
                let page = self.help_page_size();
                self.scroll_help_by(-(page as isize));
            }
            KeyCode::PageDown => {
                let page = self.help_page_size();
                self.scroll_help_by(page as isize);
            }
            KeyCode::Home => {
                self.state.help_scroll = 0;
            }
            KeyCode::End => {
                self.state.help_scroll = self.help_max_scroll();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_help_search_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('?') | KeyCode::Char('q') => {
                self.close_help();
            }
            KeyCode::Esc => {
                self.state.help_search_query.clear();
                self.state.help_scroll = 0;
                self.state.input_mode = InputMode::Help;
            }
            KeyCode::Enter => {
                self.state.input_mode = InputMode::Help;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_help_by(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_help_by(1);
            }
            KeyCode::PageUp => {
                let page = self.help_page_size();
                self.scroll_help_by(-(page as isize));
            }
            KeyCode::PageDown => {
                let page = self.help_page_size();
                self.scroll_help_by(page as isize);
            }
            KeyCode::Home => {
                self.state.help_scroll = 0;
            }
            KeyCode::End => {
                self.state.help_scroll = self.help_max_scroll();
            }
            KeyCode::Char(c) => {
                self.state.help_search_query.push(c);
                self.state.help_scroll = 0;
            }
            KeyCode::Backspace => {
                self.state.help_search_query.pop();
                self.state.help_scroll = 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn help_max_scroll(&self) -> usize {
        if self.state.help_view_height == 0 {
            return 0;
        }
        self.state
            .help_content_len
            .saturating_sub(self.state.help_view_height)
    }

    fn help_page_size(&self) -> usize {
        let page = self.state.help_view_height.saturating_sub(1);
        page.max(1)
    }

    fn scroll_help_by(&mut self, delta: isize) {
        if delta.is_negative() {
            let amount = (-delta) as usize;
            self.state.help_scroll = self.state.help_scroll.saturating_sub(amount);
        } else {
            self.state.help_scroll = self.state.help_scroll.saturating_add(delta as usize);
        }
        self.clamp_help_scroll();
    }

    fn clamp_help_scroll(&mut self) {
        let max_scroll = self.help_max_scroll();
        if self.state.help_scroll > max_scroll {
            self.state.help_scroll = max_scroll;
        }
    }

    fn open_help(&mut self) {
        self.state.show_help = true;
        self.state.input_mode = InputMode::Help;
        self.state.help_scroll = 0;
        self.state.help_search_query.clear();
        self.state.help_content_len = 0;
        self.state.help_view_height = 0;
    }

    fn close_help(&mut self) {
        self.state.show_help = false;
        self.state.input_mode = InputMode::Normal;
        self.state.help_scroll = 0;
        self.state.help_search_query.clear();
    }

    fn execute_confirmed_action(&mut self, action: ConfirmAction) -> Result<()> {
        match action {
            ConfirmAction::MarkAllRead { selected } => {
                // Queue the blocking action to be handled by the main loop with progress
                let msg = match (selected, self.state.filter.is_some()) {
                    (MarkAllOption::MarkReadAndArchive, true) => "Archiving filtered notifications",
                    (MarkAllOption::MarkReadAndArchive, false) => "Archiving notifications",
                    (MarkAllOption::MarkReadOnly, true) => "Marking filtered notifications as read",
                    (MarkAllOption::MarkReadOnly, false) => "Marking notifications as read",
                };
                self.queue_blocking_action(BlockingAction::MarkAllRead { selected }, msg);
            }
            ConfirmAction::ArchiveSelected { count: _, option } => {
                let selected_ids: Vec<String> = self.state.get_selected_notification_ids();
                self.state.clear_selection();

                // Queue the blocking action to be handled by the main loop with progress
                let msg = match option {
                    MarkAllOption::MarkReadAndArchive => "Archiving selected notifications",
                    MarkAllOption::MarkReadOnly => "Marking selected notifications as read",
                };
                self.queue_blocking_action(
                    BlockingAction::ArchiveSelected {
                        notification_ids: selected_ids,
                        option,
                    },
                    msg,
                );
            }
        }
        Ok(())
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear status message on any key press
        self.state.status_message = None;

        match key.code {
            KeyCode::Esc => {
                // First, clear selection if any
                if self.state.has_selection() {
                    self.state.clear_selection();
                } else if self.state.focused_pane != PaneFocus::None {
                    // If a pane is focused, zoom out to split view
                    self.state.focused_pane = PaneFocus::None;
                } else {
                    // Otherwise, quit
                    self.should_quit = true;
                }
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('1') => {
                // Toggle pane 1 zoom
                if self.state.focused_pane == PaneFocus::Pane1 {
                    // Already focused, zoom out
                    self.state.focused_pane = PaneFocus::None;
                } else {
                    // Zoom in to pane 1
                    self.state.focused_pane = PaneFocus::Pane1;
                }
            }
            KeyCode::Char('2') => {
                // Toggle pane 2 zoom (only if preview is enabled)
                if self.state.show_preview() {
                    if self.state.focused_pane == PaneFocus::Pane2 {
                        // Already focused, zoom out
                        self.state.focused_pane = PaneFocus::None;
                    } else {
                        // Zoom in to pane 2
                        self.state.focused_pane = PaneFocus::Pane2;
                    }
                }
                // If preview is off, do nothing
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.open_help();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_up();
                // Scroll preview to top when selection changes
                self.state.preview_scroll = 0;
                // Auto-fetch preview for the newly selected notification
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_neighbour_previews();
                }
                self.queue_auto_mark_read();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.move_down();
                // Scroll preview to top when selection changes
                self.state.preview_scroll = 0;
                // Auto-fetch preview for the newly selected notification
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_neighbour_previews();
                }
                self.queue_auto_mark_read();
            }
            KeyCode::PageUp => {
                if self.state.show_preview() {
                    // Scroll preview up by page (20 lines)
                    let page_size = 20;
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(page_size);
                } else {
                    // Move selection up if preview is not visible
                    for _ in 0..10 {
                        self.state.move_up();
                    }
                    self.state.preview_scroll = 0;
                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }
                    self.queue_auto_mark_read();
                }
            }
            KeyCode::PageDown => {
                if self.state.show_preview() {
                    // Scroll preview down by page (20 lines)
                    let page_size = 20;
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(page_size);
                } else {
                    // Move selection down if preview is not visible
                    for _ in 0..10 {
                        self.state.move_down();
                    }
                    self.state.preview_scroll = 0;
                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }
                    self.queue_auto_mark_read();
                }
            }
            KeyCode::Home => {
                self.state.selected_index = 0;
                self.state.preview_scroll = 0;
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_neighbour_previews();
                }
                self.queue_auto_mark_read();
            }
            KeyCode::End => {
                if !self.state.tree_items.is_empty() {
                    self.state.selected_index = self.state.tree_items.len() - 1;
                }
                self.state.preview_scroll = 0;
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_neighbour_previews();
                }
                self.queue_auto_mark_read();
            }
            KeyCode::Enter => {
                if self.state.has_selection() {
                    // Open all selected notifications and mark as read
                    let selected_ids = self.state.get_selected_notification_ids();
                    let mut marked_count = 0;

                    let urls: Vec<String> = selected_ids
                        .iter()
                        .filter_map(|id| {
                            self.state
                                .notifications
                                .iter()
                                .find(|n| &n.id == id)
                                .and_then(|n| {
                                    self.discussion_url_from_preview(n)
                                        .or_else(|| n.web_url(&self.config.github_host))
                                })
                        })
                        .collect();
                    let opened_count = urls.len();
                    for url in &urls {
                        self.deliver_url(url);
                    }

                    for notification_id in &selected_ids {
                        if self.auto_mark_on_open {
                            if let Some(notif) = self
                                .state
                                .notifications
                                .iter()
                                .find(|n| &n.id == notification_id)
                            {
                                if notif.is_unread() {
                                    marked_count += 1;
                                }
                            }
                            self.state.mark_notification_read(notification_id);
                            if is_synthetic_id(notification_id) {
                                let _ = AppStateFile::dismiss_synthetic_id(notification_id);
                            } else if let Some(ref client) = self.api_client {
                                let _ = client.mark_notification_read(notification_id);
                            }
                        }
                    }

                    self.state.clear_selection();
                    let verb = self.open_method_verb();
                    self.state.status_message = if self.auto_mark_on_open {
                        Some(format!(
                            "{} {} notification{}, marked {} as read",
                            verb,
                            opened_count,
                            if opened_count == 1 { "" } else { "s" },
                            marked_count
                        ))
                    } else {
                        Some(format!(
                            "{} {} notification{}",
                            verb,
                            opened_count,
                            if opened_count == 1 { "" } else { "s" }
                        ))
                    };
                } else if let Some(org_name) = self.state.selected_org() {
                    let org_name = org_name.to_string();
                    self.state.toggle_org_expansion(&org_name);
                } else if let Some(repo_name) = self.state.selected_repo() {
                    // If selected item is a repository header, toggle expansion
                    let repo_name = repo_name.to_string();
                    self.state.toggle_repo_expansion(&repo_name);
                } else if let Some(notification) = self.state.selected_notification().cloned() {
                    // Open the notification URL
                    self.open_notification_url(&notification);

                    // Mark notification as read if it's unread and auto_mark_on_open is enabled
                    if self.auto_mark_on_open && notification.is_unread() {
                        let notification_id = notification.id.clone();

                        // Update local state optimistically (for better UX)
                        self.state.mark_notification_read(&notification_id);

                        // Persist read state
                        if is_synthetic_id(&notification_id) {
                            let _ = AppStateFile::dismiss_synthetic_id(&notification_id);
                        } else if let Some(ref client) = self.api_client {
                            if let Err(e) = client.mark_notification_read(&notification_id) {
                                eprintln!("Failed to mark notification as read: {}", e);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('o') => {
                if self.state.has_selection() {
                    let selected_ids = self.state.get_selected_notification_ids();
                    let urls: Vec<String> = selected_ids
                        .iter()
                        .filter_map(|id| {
                            self.state
                                .notifications
                                .iter()
                                .find(|n| &n.id == id)
                                .and_then(|n| {
                                    self.discussion_url_from_preview(n)
                                        .or_else(|| n.web_url(&self.config.github_host))
                                })
                        })
                        .collect();
                    let count = urls.len();
                    for url in &urls {
                        self.deliver_url(url);
                    }
                    self.state.clear_selection();
                    let verb = self.open_method_verb();
                    self.state.status_message = Some(format!(
                        "{} {} notification{}",
                        verb,
                        count,
                        if count == 1 { "" } else { "s" }
                    ));
                } else if let Some(notification) = self.state.selected_notification().cloned() {
                    self.open_notification_url(&notification);
                }
            }
            KeyCode::Char('O') => {
                if self.state.has_selection() || self.state.selected_notification().is_some() {
                    self.state.url_menu_index = 0;
                    self.state.input_mode = InputMode::UrlMenu;
                }
            }
            KeyCode::Char(' ') => {
                // Space bar toggles multi-select on notifications
                if let Some(notification) = self.state.selected_notification() {
                    let notification_id = notification.id.clone();
                    self.state.toggle_selection(notification_id);
                    self.state.move_down(); // Auto-advance to next notification
                } else if let Some(repo_name) = self.state.selected_repo() {
                    // On repo headers, toggle expansion (legacy behaviour)
                    let repo_name = repo_name.to_string();
                    self.state.toggle_repo_expansion(&repo_name);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // Collapse org if on an org header
                if let Some(org_name) = self.state.selected_org() {
                    let org_name = org_name.to_string();
                    let is_expanded = self
                        .state
                        .expanded_orgs
                        .get(&org_name)
                        .copied()
                        .unwrap_or(true);
                    if is_expanded {
                        self.state.expanded_orgs.insert(org_name, false);
                        self.state.build_tree();
                        if !self.state.tree_items.is_empty() {
                            self.state.selected_index = self
                                .state
                                .selected_index
                                .min(self.state.tree_items.len() - 1);
                        }
                    }
                } else if let Some(repo_name) = self.state.parent_repo_for_selected() {
                    let repo_name = repo_name.to_string();
                    // Check if it's currently expanded (default is expanded)
                    let is_expanded = self
                        .state
                        .expanded_repos
                        .get(&repo_name)
                        .copied()
                        .unwrap_or(true);
                    if is_expanded {
                        // Collapse it
                        self.state.expanded_repos.insert(repo_name, false);
                        self.state.build_tree();
                        // Adjust selected_index if needed
                        if !self.state.tree_items.is_empty() {
                            self.state.selected_index = self
                                .state
                                .selected_index
                                .min(self.state.tree_items.len() - 1);
                        }
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                // Expand org if on a collapsed org header
                if let Some(org_name) = self.state.selected_org() {
                    let org_name = org_name.to_string();
                    let is_expanded = self
                        .state
                        .expanded_orgs
                        .get(&org_name)
                        .copied()
                        .unwrap_or(true);
                    if !is_expanded {
                        self.state.expanded_orgs.insert(org_name, true);
                        self.state.build_tree();
                    }
                } else if let Some(repo_name) = self.state.selected_repo() {
                    let repo_name = repo_name.to_string();
                    let is_expanded = self
                        .state
                        .expanded_repos
                        .get(&repo_name)
                        .copied()
                        .unwrap_or(true);
                    if !is_expanded {
                        self.state.expanded_repos.insert(repo_name, true);
                        self.state.build_tree();
                    }
                }
            }
            KeyCode::Char('.') | KeyCode::Char('r')
                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let advance = key.code == KeyCode::Char('.');
                if self.state.has_selection() {
                    // Toggle read status of all selected notifications
                    let selected_ids = self.state.get_selected_notification_ids();
                    let count = selected_ids.len();
                    let mut marked_read = 0;
                    let mut marked_unread = 0;

                    for notification_id in &selected_ids {
                        // Check current state before toggle
                        let was_unread = self
                            .state
                            .notifications
                            .iter()
                            .find(|n| n.id == *notification_id)
                            .map(|n| n.is_unread())
                            .unwrap_or(false);

                        if let Some(is_now_unread) =
                            self.state.toggle_notification_read(notification_id)
                        {
                            if was_unread && !is_now_unread {
                                // Went from unread to read
                                marked_read += 1;
                                if is_synthetic_id(notification_id) {
                                    let _ = AppStateFile::dismiss_synthetic_id(notification_id);
                                } else if let Some(ref client) = self.api_client {
                                    if let Err(e) = client.mark_notification_read(notification_id) {
                                        eprintln!(
                                            "Failed to mark notification {} as read: {}",
                                            notification_id, e
                                        );
                                    }
                                }
                            } else if !was_unread && is_now_unread {
                                // Went from read to unread - local only
                                marked_unread += 1;
                            }
                        }
                    }

                    self.state.clear_selection();

                    // Build status message based on what happened
                    let msg = if marked_read > 0 && marked_unread > 0 {
                        format!(
                            "Toggled {} notifications ({} read, {} unread)",
                            count, marked_read, marked_unread
                        )
                    } else if marked_read > 0 {
                        format!("Marked {} notifications as read", marked_read)
                    } else if marked_unread > 0 {
                        format!("Marked {} notifications as unread", marked_unread)
                    } else {
                        format!("Toggled {} notifications", count)
                    };
                    self.state.status_message = Some(msg);
                } else if let Some(notification) = self.state.selected_notification() {
                    // Toggle read/unread status of single notification
                    let notification_id = notification.id.clone();
                    let was_unread = notification.is_unread();

                    // Toggle local state
                    if let Some(is_now_unread) =
                        self.state.toggle_notification_read(&notification_id)
                    {
                        // If marking as read (was unread, now read), persist the change
                        if was_unread && !is_now_unread {
                            if is_synthetic_id(&notification_id) {
                                let _ = AppStateFile::dismiss_synthetic_id(&notification_id);
                            } else if let Some(ref client) = self.api_client {
                                if let Err(e) = client.mark_notification_read(&notification_id) {
                                    eprintln!("Failed to mark notification as read: {}", e);
                                    // Revert local state on API failure
                                    self.state.toggle_notification_read(&notification_id);
                                }
                            }
                            if advance {
                                // Move to next notification
                                self.state.move_down();
                                // Scroll preview to top when selection changes
                                self.state.preview_scroll = 0;
                                // Auto-fetch preview for the newly selected notification
                                if self.state.show_preview() {
                                    self.fetch_preview_for_selected_notification();
                                    self.prefetch_neighbour_previews();
                                }
                            }
                        }
                        // If marking as unread (was read, now unread), just update local state
                        // Note: GitHub API doesn't support marking as unread, so this won't persist on refresh
                    }
                }
            }
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.state.has_selection() {
                    // Archive all selected notifications
                    let selected_ids = self.state.get_selected_notification_ids();
                    let count = selected_ids.len();

                    let synthetic_ids: Vec<String> = selected_ids
                        .iter()
                        .filter(|id| is_synthetic_id(id))
                        .cloned()
                        .collect();
                    if !synthetic_ids.is_empty() {
                        let _ = AppStateFile::dismiss_synthetic_ids(&synthetic_ids);
                    }

                    if let Some(ref client) = self.api_client {
                        for notification_id in &selected_ids {
                            if !is_synthetic_id(notification_id) {
                                if let Err(e) = client.mark_thread_done(notification_id) {
                                    eprintln!(
                                        "Failed to archive notification {}: {}",
                                        notification_id, e
                                    );
                                }
                            }
                        }
                    }

                    let saved_index = self.state.selected_index;
                    self.state.remove_notifications(&selected_ids);
                    self.state.clear_selection();

                    // Stay near the same position
                    if !self.state.tree_items.is_empty() {
                        self.state.selected_index =
                            saved_index.min(self.state.tree_items.len() - 1);
                        if !matches!(
                            self.state.tree_items.get(self.state.selected_index),
                            Some(crate::state::TreeItem::Notification(_))
                        ) {
                            self.state.select_first_notification();
                        }
                    }

                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }

                    self.state.status_message = Some(format!("Archived {} notifications", count));
                } else if let Some(notification) = self.state.selected_notification() {
                    let notification_id = notification.id.clone();
                    let saved_index = self.state.selected_index;

                    // Skip API call for synthetic notifications (no real thread)
                    if is_synthetic_id(&notification_id) {
                        let _ = AppStateFile::dismiss_synthetic_id(&notification_id);
                    } else if let Some(ref client) = self.api_client {
                        if let Err(e) = client.mark_thread_done(&notification_id) {
                            eprintln!("Failed to archive notification: {}", e);
                        }
                    }

                    self.state.remove_notification(&notification_id);

                    // Stay at the same position (or clamp to end of list)
                    if !self.state.tree_items.is_empty() {
                        self.state.selected_index =
                            saved_index.min(self.state.tree_items.len() - 1);
                        // Skip headers — find nearest notification
                        if !matches!(
                            self.state.tree_items.get(self.state.selected_index),
                            Some(crate::state::TreeItem::Notification(_))
                        ) {
                            self.state.select_first_notification();
                        }
                    }

                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }

                    self.state.status_message = Some("Archived notification".to_string());
                }
            }
            KeyCode::Char('!') => {
                if self.state.has_selection() {
                    // Toggle pin status of all selected notifications
                    let selected_ids = self.state.get_selected_notification_ids();
                    let count = selected_ids.len();
                    let mut pinned = 0;
                    let mut unpinned = 0;

                    for notification_id in &selected_ids {
                        // Find the notification by ID and clone it
                        if let Some(notification) = self
                            .state
                            .notifications
                            .iter()
                            .find(|n| n.id == *notification_id)
                            .cloned()
                        {
                            let was_pinned = self.state.is_pinned(&notification.id);
                            self.state.toggle_pin(notification);
                            if was_pinned {
                                unpinned += 1;
                            } else {
                                pinned += 1;
                            }
                        }
                    }

                    self.pinned_state_dirty = true;
                    self.state.build_tree();
                    self.state.clear_selection();
                    self.state.select_first_notification();

                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }

                    // Build status message
                    let msg = if pinned > 0 && unpinned > 0 {
                        format!(
                            "Toggled {} pins ({} pinned, {} unpinned)",
                            count, pinned, unpinned
                        )
                    } else if pinned > 0 {
                        format!("Pinned {} notifications", pinned)
                    } else {
                        format!("Unpinned {} notifications", unpinned)
                    };
                    self.state.status_message = Some(msg);
                } else if let Some(notification) = self.state.selected_notification() {
                    // Toggle pin status of single notification
                    let selected_notif_id = notification.id.clone();
                    let notification = notification.clone();

                    self.state.toggle_pin(notification);

                    // Mark state as dirty instead of immediate save
                    self.pinned_state_dirty = true;

                    // Rebuild tree to move notification to/from pinned section
                    self.state.build_tree();

                    // Restore selection to same notification
                    if !self.state.select_notification_by_id(&selected_notif_id) {
                        self.state.select_first_notification();
                    }

                    // Refresh preview for selected notification
                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_neighbour_previews();
                    }
                }
            }
            KeyCode::Tab => {
                // Cycle through preview modes: Off -> Horizontal -> Vertical -> Off
                self.state.preview_mode = match self.state.preview_mode {
                    PreviewMode::Off => PreviewMode::Horizontal,
                    PreviewMode::Horizontal => PreviewMode::Vertical,
                    PreviewMode::Vertical => PreviewMode::Off,
                };
                self.state.preview_scroll = 0;

                // When the preview pane becomes visible, always (re-)fetch for the selected
                // notification.  This ensures stale content is never silently reused and that
                // a notification invalidated while preview was Off triggers revalidation now.
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_neighbour_previews();
                }
            }
            KeyCode::Char('M') => {
                self.auto_mark_read_enabled = !self.auto_mark_read_enabled;
                if !self.auto_mark_read_enabled {
                    self.pending_mark_read = None;
                }
                let _ = crate::state_file::AppStateFile::save_auto_mark_read(
                    self.auto_mark_read_enabled,
                );
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Scroll preview up
                self.state.preview_scroll = self.state.preview_scroll.saturating_sub(5);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Scroll preview down
                self.state.preview_scroll = self.state.preview_scroll.saturating_add(5);
            }
            KeyCode::Char('K') => {
                // Scroll preview up (capital K)
                self.state.preview_scroll = self.state.preview_scroll.saturating_sub(1);
            }
            KeyCode::Char('J') => {
                // Scroll preview down (capital J)
                self.state.preview_scroll = self.state.preview_scroll.saturating_add(1);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Scroll preview up by page (20 lines)
                if self.state.show_preview() {
                    let page_size = 20;
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(page_size);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Scroll preview down by page (20 lines)
                if self.state.show_preview() {
                    let page_size = 20;
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(page_size);
                }
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Force refresh notifications
                self.queue_blocking_action(BlockingAction::Refresh, "Refreshing notifications...");
            }
            KeyCode::Char('E') | KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.expunge_read_notifications();
            }
            KeyCode::Char('A') | KeyCode::Char('a')
                if key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                // Toggle showing read notifications
                self.state.show_all = !self.state.show_all;
                if let Some((_, participating, max_notifications)) = self.refresh_args {
                    self.refresh_args =
                        Some((self.state.show_all, participating, max_notifications));
                }
                self.queue_blocking_action(BlockingAction::Refresh, "Refreshing notifications...");
            }
            KeyCode::Char('a')
                if key
                    .modifiers
                    .contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(repo_name) = self.state.parent_repo_for_selected() {
                    let repo_name = repo_name.to_string();
                    let repo_selected = self.state.toggle_select_all_in_repo(&repo_name);
                    if repo_selected == 0 {
                        self.state.status_message =
                            Some(format!("Cleared selection in {}", repo_name));
                    } else {
                        self.state.status_message = Some(format!(
                            "Selected {} notification{} in {}",
                            repo_selected,
                            if repo_selected == 1 { "" } else { "s" },
                            repo_name
                        ));
                    }
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.state.has_selection() {
                    // Archive selected notifications
                    let count = self.state.selection_count();
                    self.state.confirm_action = Some(ConfirmAction::ArchiveSelected {
                        count,
                        option: MarkAllOption::MarkReadAndArchive, // Default to archive
                    });
                    self.state.input_mode = InputMode::Confirm;
                } else if !self.state.notifications.is_empty() {
                    // Show confirmation dialog for mark all as read
                    self.state.confirm_action = Some(ConfirmAction::MarkAllRead {
                        selected: MarkAllOption::MarkReadOnly, // Default
                    });
                    self.state.input_mode = InputMode::Confirm;
                }
            }
            KeyCode::Char('/') => {
                // Enter interactive filter mode, clear selection
                self.state.clear_selection();
                self.state.search_query.clear();
                self.state.input_mode = InputMode::Search;
            }
            KeyCode::Char('x') => {
                // Open action menu (built-in actions are always available)
                // Only open if there's a notification selected or multi-select is active
                if self.state.has_selection() || self.state.selected_notification().is_some() {
                    self.state.action_menu_index = 0;
                    self.state.input_mode = InputMode::ActionMenu;
                }
            }
            KeyCode::Char('V') => {
                self.state.view_picker_index = 0;
                self.state.input_mode = InputMode::ViewPicker;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Exit search mode; restore view filter if one is active
                self.state.input_mode = InputMode::Normal;
                self.state.search_query.clear();
                self.refresh_filter_state()?;
            }
            KeyCode::Enter => {
                // Keep current filter and exit search mode
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.state.search_query.push(c);
                self.apply_search_filter();
            }
            KeyCode::Backspace => {
                self.state.search_query.pop();
                self.apply_search_filter();
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_search_filter(&mut self) {
        if let Err(err) = self.refresh_filter_state() {
            self.state.status_message = Some(format!("Search filter error: {}", err));
        }
    }

    fn expunge_read_notifications(&mut self) {
        if self.state.show_all {
            self.state.show_all = false;
            if let Some((_, participating, max_notifications)) = self.refresh_args {
                self.refresh_args = Some((false, participating, max_notifications));
            }
        }

        let selected_id = self.state.selected_notification().map(|n| n.id.clone());
        let mut removed = 0usize;
        let mut kept = Vec::with_capacity(self.state.notifications.len());

        for notification in &self.state.notifications {
            if notification.is_unread() || self.state.is_pinned(&notification.id) {
                kept.push(notification.clone());
            } else {
                removed += 1;
            }
        }

        if removed == 0 {
            self.state.status_message = Some("No read notifications to expunge".to_string());
            return;
        }

        self.state.set_notifications(kept);

        let existing_ids: HashSet<String> = self
            .state
            .notifications
            .iter()
            .map(|n| n.id.clone())
            .collect();
        self.state
            .selected_notification_ids
            .retain(|id| existing_ids.contains(id));

        if let Some(notification_id) = selected_id {
            if !self.state.select_notification_by_id(&notification_id) {
                self.state.select_first_notification();
            }
        } else {
            self.state.select_first_notification();
        }

        self.state.preview_scroll = 0;
        if self.state.show_preview() {
            self.fetch_preview_for_selected_notification();
            self.prefetch_neighbour_previews();
        }

        self.pending_mark_read = None;

        self.state.status_message = Some(format!(
            "Expunged {} read notification{}",
            removed,
            if removed == 1 { "" } else { "s" }
        ));

        if self.refresh_args.is_some() {
            self.queue_blocking_action(BlockingAction::Refresh, "Refreshing notifications...");
        }
    }

    fn handle_mouse(&mut self, mouse_event: MouseEvent, size: Rect) -> Result<()> {
        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(mouse_event.column, mouse_event.row, size)?;
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll(
                    mouse_event.column,
                    mouse_event.row,
                    mouse_event.kind,
                    size,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn effective_preview_mode(&self) -> PreviewMode {
        if self.state.show_preview() {
            self.state.preview_mode
        } else {
            PreviewMode::Off
        }
    }

    fn get_list_area(&self, size: Rect) -> Option<Rect> {
        // Account for status bar (1 line at top)
        if size.height < 2 {
            return None;
        }

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(0),    // Main content
            ])
            .split(size);

        let main_area = main_chunks[1];

        match self.state.focused_pane {
            PaneFocus::Pane1 => {
                // Entire main area is list
                Some(main_area)
            }
            PaneFocus::Pane2 => {
                // If preview is off, main area is list, otherwise it's preview
                if self.state.show_preview() {
                    None
                } else {
                    Some(main_area)
                }
            }
            PaneFocus::None => {
                // Split view - determine list area based on preview_mode
                match self.effective_preview_mode() {
                    PreviewMode::Off => Some(main_area),
                    PreviewMode::Horizontal => {
                        let chunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([
                                Constraint::Ratio(1, 3), // List: 1/3 of width
                                Constraint::Ratio(2, 3), // Preview: 2/3 of width
                            ])
                            .split(main_area);
                        Some(chunks[0])
                    }
                    PreviewMode::Vertical => {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Ratio(1, 3), // List: 1/3 of height
                                Constraint::Ratio(2, 3), // Preview: 2/3 of height
                            ])
                            .split(main_area);
                        Some(chunks[0])
                    }
                }
            }
        }
    }

    fn get_list_item_at_position(&self, column: u16, row: u16, size: Rect) -> Option<usize> {
        // Get the list widget area
        let list_area = self.get_list_area(size)?;

        // Check if click is within list area bounds
        if column < list_area.x
            || column >= list_area.x + list_area.width
            || row < list_area.y
            || row >= list_area.y + list_area.height
        {
            return None;
        }

        // List widget has:
        // - Borders: 1 line top + 1 line bottom = 2 lines total
        // - Padding: 1 line on all sides
        // So the inner content area starts at: list_area.y + 1 (top border) + 1 (top padding) = list_area.y + 2
        let border_top = 1;
        let padding_top = 1;
        let inner_start_y = list_area.y + border_top + padding_top;

        // Check if click is within inner content area (not on borders or padding)
        if row < inner_start_y {
            return None; // Click is on top border or padding
        }

        // Calculate relative row within inner content area
        let row_in_inner = row - inner_start_y;
        let inner_height = list_area
            .height
            .saturating_sub(border_top + padding_top + 1 + 1); // top border + top padding + bottom padding + bottom border

        if row_in_inner >= inner_height {
            return None; // Click is on bottom padding or border
        }

        // Now we need to figure out which item is at this row
        // The List widget automatically scrolls to keep the selected item visible
        // We need to calculate the scroll offset based on the selected index

        if self.state.tree_items.is_empty() {
            return None;
        }

        let selected_idx = self
            .state
            .selected_index
            .min(self.state.tree_items.len().saturating_sub(1));
        let total_items = self.state.tree_items.len();
        let inner_height_usize = inner_height as usize;

        // Calculate the first visible item index
        // The List widget tries to keep the selected item visible
        // Typical behavior: position selected item at top of viewport if possible
        // Otherwise, scroll to keep it visible

        let first_visible_idx = if total_items <= inner_height_usize {
            // All items fit in viewport, start at 0
            0
        } else if selected_idx < inner_height_usize {
            // Selected item is in the first screenful, start at 0
            0
        } else {
            // Selected item is further down, calculate offset to keep it visible
            // We want to show the selected item, so we calculate:
            // first_visible = max(0, selected_idx - (inner_height - 1))
            // This positions selected at the bottom of viewport
            // But we also need to ensure we don't go past the end
            let calculated_offset =
                selected_idx.saturating_sub(inner_height_usize.saturating_sub(1));
            let max_offset = total_items.saturating_sub(inner_height_usize);
            calculated_offset.min(max_offset).max(0)
        };

        // Calculate the clicked item index
        let clicked_item_idx = first_visible_idx + row_in_inner as usize;

        // Make sure the index is valid
        if clicked_item_idx < total_items {
            Some(clicked_item_idx)
        } else {
            None
        }
    }

    fn get_pane_at_position(&self, size: Rect, column: u16, row: u16) -> Option<PaneFocus> {
        // Check if click is in status bar (row 0)
        if row < 1 {
            return None;
        }

        // Account for status bar (1 line)
        let main_area_y = row.saturating_sub(1);
        let main_area_height = size.height.saturating_sub(1);

        // Check if click is within main area bounds
        if main_area_y >= main_area_height || column >= size.width {
            return None;
        }

        match self.state.focused_pane {
            PaneFocus::Pane1 => {
                // Entire main area is Pane1
                Some(PaneFocus::Pane1)
            }
            PaneFocus::Pane2 => {
                // Entire main area is Pane2 (if preview is enabled)
                if self.state.show_preview() {
                    Some(PaneFocus::Pane2)
                } else {
                    Some(PaneFocus::Pane1)
                }
            }
            PaneFocus::None => {
                // Split view - determine which pane based on preview_mode
                match self.effective_preview_mode() {
                    PreviewMode::Off => {
                        // Only Pane1 exists
                        Some(PaneFocus::Pane1)
                    }
                    PreviewMode::Horizontal => {
                        // Split horizontally: List (1/3) | Preview (2/3)
                        let list_width = (size.width as f32 * (1.0 / 3.0)) as u16;
                        if column < list_width {
                            Some(PaneFocus::Pane1)
                        } else {
                            Some(PaneFocus::Pane2)
                        }
                    }
                    PreviewMode::Vertical => {
                        // Split vertically: List (1/3) | Preview (2/3)
                        let list_height = (main_area_height as f32 * (1.0 / 3.0)) as u16;
                        if main_area_y < list_height {
                            Some(PaneFocus::Pane1)
                        } else {
                            Some(PaneFocus::Pane2)
                        }
                    }
                }
            }
        }
    }

    fn handle_mouse_click(&mut self, column: u16, row: u16, size: Rect) -> Result<()> {
        if let Some(clicked_pane) = self.get_pane_at_position(size, column, row) {
            match clicked_pane {
                PaneFocus::Pane1 => {
                    // Try to get the clicked list item
                    if let Some(clicked_item_idx) =
                        self.get_list_item_at_position(column, row, size)
                    {
                        // Check if clicked item is a notification (not a repository header)
                        if let Some(crate::state::TreeItem::Notification(_)) =
                            self.state.tree_items.get(clicked_item_idx)
                        {
                            // Clicked on a notification - select it and show preview
                            self.state.selected_index = clicked_item_idx;
                            self.state.preview_scroll = 0;

                            // Fetch and show preview for the selected notification
                            if self.state.show_preview() {
                                self.fetch_preview_for_selected_notification();
                                self.prefetch_neighbour_previews();
                            } else {
                                // Enable preview if it's off
                                self.state.preview_mode = PreviewMode::Horizontal;
                                self.fetch_preview_for_selected_notification();
                                self.prefetch_neighbour_previews();
                            }
                            self.queue_auto_mark_read();
                        } else if let Some(crate::state::TreeItem::OrgHeader(org_info)) =
                            self.state.tree_items.get(clicked_item_idx)
                        {
                            let org_name = org_info.login.clone();
                            self.state.toggle_org_expansion(&org_name);
                        } else if let Some(crate::state::TreeItem::RepositoryHeader(repo_info)) =
                            self.state.tree_items.get(clicked_item_idx)
                        {
                            // Clicked on a repository header - toggle expansion
                            let repo_name = repo_info.full_name.clone();
                            self.state.toggle_repo_expansion(&repo_name);

                            // After toggling, try to select the first notification in this repo
                            if let Some(first_notif_idx) = self
                                .state
                                .tree_items
                                .iter()
                                .enumerate()
                                .find_map(|(idx, item)| {
                                    if let crate::state::TreeItem::Notification(notif_idx) = item {
                                        if let Some(notif) =
                                            self.state.notifications.get(*notif_idx)
                                        {
                                            if notif.repo_full_name() == repo_name {
                                                return Some(idx);
                                            }
                                        }
                                    }
                                    None
                                })
                            {
                                self.state.selected_index = first_notif_idx;
                                self.state.preview_scroll = 0;
                                if self.state.show_preview() {
                                    self.fetch_preview_for_selected_notification();
                                    self.prefetch_neighbour_previews();
                                }
                                self.queue_auto_mark_read();
                            }
                        } else {
                            // Clicked outside list items (on borders/padding) - toggle zoom
                            if self.state.focused_pane == PaneFocus::Pane1 {
                                self.state.focused_pane = PaneFocus::None;
                            } else {
                                self.state.focused_pane = PaneFocus::Pane1;
                            }
                        }
                    } else {
                        // Clicked outside list area or on borders - toggle zoom
                        if self.state.focused_pane == PaneFocus::Pane1 {
                            self.state.focused_pane = PaneFocus::None;
                        } else {
                            self.state.focused_pane = PaneFocus::Pane1;
                        }
                    }
                }
                PaneFocus::Pane2 => {
                    // Only allow focusing Pane2 if preview is enabled
                    if self.state.show_preview() {
                        if self.state.focused_pane == PaneFocus::Pane2 {
                            // Already focused, zoom out
                            self.state.focused_pane = PaneFocus::None;
                        } else {
                            // Focus Pane2
                            self.state.focused_pane = PaneFocus::Pane2;
                        }
                    }
                }
                PaneFocus::None => {
                    // Should not happen from get_pane_at_position
                }
            }
        }
        Ok(())
    }

    fn handle_mouse_scroll(
        &mut self,
        column: u16,
        row: u16,
        kind: MouseEventKind,
        size: Rect,
    ) -> Result<()> {
        if let Some(scrolled_pane) = self.get_pane_at_position(size, column, row) {
            match scrolled_pane {
                PaneFocus::Pane1 => {
                    // Use ratatui's native ListState scrolling methods
                    let max_items = self.state.tree_items.len();
                    match kind {
                        MouseEventKind::ScrollUp => {
                            self.list_widget.scroll_up(max_items);
                        }
                        MouseEventKind::ScrollDown => {
                            self.list_widget.scroll_down(max_items);
                        }
                        _ => {}
                    }

                    // Sync ListState selection back to AppState
                    if let Some(selected_idx) = self.list_widget.selected() {
                        if selected_idx < self.state.tree_items.len() {
                            self.state.selected_index = selected_idx;
                            // Fetch preview for the newly selected notification
                            if self.state.show_preview() {
                                self.fetch_preview_for_selected_notification();
                                self.prefetch_neighbour_previews();
                            }
                            self.queue_auto_mark_read();
                        }
                    }
                }
                PaneFocus::Pane2 => {
                    // Scroll preview pane
                    if self.state.show_preview() {
                        match kind {
                            MouseEventKind::ScrollUp => {
                                self.state.preview_scroll =
                                    self.state.preview_scroll.saturating_sub(1);
                            }
                            MouseEventKind::ScrollDown => {
                                // Increment scroll - the preview widget will clamp it to valid bounds when rendering
                                self.state.preview_scroll =
                                    self.state.preview_scroll.saturating_add(1);
                            }
                            _ => {}
                        }
                    }
                }
                PaneFocus::None => {
                    // Should not happen from get_pane_at_position
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::GitHubClient;
    use crate::config::{OpenMethod, View};
    use crate::models::{Notification, NotificationType, Owner, Repository, Subject};

    fn test_notification(id: &str, unread: bool) -> Notification {
        Notification {
            id: id.to_string(),
            unread,
            last_read_at: None,
            updated_at: None,
            reason: "mention".to_string(),
            repository: Repository {
                id: 1,
                name: "repo".to_string(),
                full_name: "owner/repo".to_string(),
                owner: Owner {
                    login: "owner".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: "Test notification".to_string(),
                subject_type: NotificationType::Issue,
                url: Some("https://github.com/owner/repo/issues/1".to_string()),
                latest_comment_url: None,
            },
            latest_comment_url: None,
            author: None,
            context: None,
        }
    }

    fn notification_with(
        id: &str,
        title: &str,
        reason: &str,
        latest_comment_url: Option<&str>,
    ) -> Notification {
        Notification {
            id: id.to_string(),
            unread: true,
            last_read_at: None,
            updated_at: None,
            reason: reason.to_string(),
            repository: Repository {
                id: 1,
                name: "repo".to_string(),
                full_name: "owner/repo".to_string(),
                owner: Owner {
                    login: "owner".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: title.to_string(),
                subject_type: NotificationType::Issue,
                url: Some("https://github.com/owner/repo/issues/1".to_string()),
                latest_comment_url: latest_comment_url.map(str::to_string),
            },
            latest_comment_url: latest_comment_url.map(str::to_string),
            author: None,
            context: None,
        }
    }

    #[test]
    fn process_pending_mark_read_marks_synthetic_locally() {
        let mut app = App::new(Config::default());
        app.state
            .set_notifications(vec![test_notification("actions-123", true)]);
        app.pending_mark_read = Some((
            "actions-123".to_string(),
            Instant::now() - Duration::from_millis(AUTO_MARK_READ_DWELL_MS + 1),
        ));

        app.process_pending_mark_read();

        // Local state should be updated (no API client, so no API call)
        assert!(!app.state.notifications[0].is_unread());
        assert!(app.pending_mark_read.is_none());
    }

    #[test]
    fn handle_normal_key_toggles_synthetic_notifications_locally() {
        let mut app = App::new(Config::default());
        app.state
            .set_notifications(vec![test_notification("actions-123", true)]);

        app.handle_normal_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE))
            .unwrap();

        // Local state should be toggled (no API call for synthetic)
        assert!(!app.state.notifications[0].is_unread());
    }

    #[test]
    fn handle_normal_key_archives_synthetic_notifications_locally() {
        let mut app = App::new(Config::default());
        app.state
            .set_notifications(vec![test_notification("event-123", true)]);

        app.handle_normal_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE))
            .unwrap();

        // Synthetic notification should be removed locally (no API call)
        assert_eq!(app.state.notifications.len(), 0);
    }

    #[test]
    fn search_keeps_active_view_constraints() {
        let mut app = App::new(Config::default());
        app.state.views = vec![View {
            name: "Participating".to_string(),
            filter: None,
            exclude_types: None,
            exclude_reasons: Some(vec!["subscribed".to_string()]),
            exclude_repos: None,
            exclude_subjects: None,
        }];
        app.state.set_notifications(vec![
            notification_with("1", "Alpha thread", "subscribed", None),
            notification_with("2", "Alpha thread", "mention", None),
        ]);

        app.apply_view(1).unwrap();
        assert_eq!(app.state.filtered_notifications.len(), 1);

        app.state.search_query = "Alpha".to_string();
        app.apply_search_filter();

        assert_eq!(app.state.filtered_notifications.len(), 1);
        assert_eq!(app.state.active_view_index, Some(0));
        let visible_idx = app.state.filtered_notifications[0];
        assert_eq!(app.state.notifications[visible_idx].id, "2");
    }

    #[test]
    fn clearing_view_restores_runtime_base_filter() {
        let mut app = App::new(Config::default());
        app.base_filter = Some(Filter::from_pattern(Some("CLI only")).unwrap());
        app.base_filter_pattern = Some("CLI only".to_string());
        app.state.views = vec![View {
            name: "Mentions".to_string(),
            filter: Some("mention$".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        }];
        app.state.set_notifications(vec![
            notification_with("1", "CLI only", "subscribed", None),
            notification_with("2", "Different", "mention", None),
        ]);
        app.state.set_filter(app.base_filter.clone());
        app.state
            .set_filter_pattern(app.base_filter_pattern.clone());

        app.apply_view(1).unwrap();
        app.apply_view(0).unwrap();

        assert_eq!(app.state.filtered_notifications.len(), 1);
        let visible_idx = app.state.filtered_notifications[0];
        assert_eq!(app.state.notifications[visible_idx].id, "1");
        assert_eq!(app.state.filter_pattern.as_deref(), Some("CLI only"));
    }

    #[test]
    fn exiting_search_restores_runtime_base_filter() {
        let mut app = App::new(Config::default());
        app.base_filter = Some(Filter::from_pattern(Some("CLI only")).unwrap());
        app.base_filter_pattern = Some("CLI only".to_string());
        app.state.set_notifications(vec![
            notification_with("1", "CLI only", "mention", None),
            notification_with("2", "Other", "mention", None),
        ]);
        app.state.set_filter(app.base_filter.clone());
        app.state
            .set_filter_pattern(app.base_filter_pattern.clone());
        app.state.input_mode = InputMode::Search;
        app.state.search_query = "Other".to_string();
        app.apply_search_filter();

        app.handle_search_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();

        assert_eq!(app.state.filtered_notifications.len(), 1);
        let visible_idx = app.state.filtered_notifications[0];
        assert_eq!(app.state.notifications[visible_idx].id, "1");
        assert_eq!(app.state.filter_pattern.as_deref(), Some("CLI only"));
    }

    #[test]
    fn invalid_view_preserves_existing_filter() {
        let mut app = App::new(Config::default());
        app.base_filter = Some(Filter::from_pattern(Some("Alpha")).unwrap());
        app.base_filter_pattern = Some("Alpha".to_string());
        app.state.views = vec![View {
            name: "Broken".to_string(),
            filter: Some("[".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        }];
        app.state.set_notifications(vec![
            notification_with("1", "Alpha", "mention", None),
            notification_with("2", "Beta", "mention", None),
        ]);
        app.state.set_filter(app.base_filter.clone());
        app.state
            .set_filter_pattern(app.base_filter_pattern.clone());

        app.apply_view(1).unwrap();

        assert_eq!(app.state.active_view_index, None);
        assert_eq!(app.state.filtered_notifications.len(), 1);
        let visible_idx = app.state.filtered_notifications[0];
        assert_eq!(app.state.notifications[visible_idx].id, "1");
        assert!(app
            .state
            .status_message
            .as_deref()
            .unwrap_or_default()
            .contains("Broken"));
    }

    #[test]
    fn refresh_restarts_author_enrichment() {
        let mut app = App::new(Config::default());
        app.set_api_client(GitHubClient::new_test());

        app.merge_refreshed_notifications(vec![notification_with(
            "1",
            "Needs author",
            "mention",
            Some("http://127.0.0.1/comment"),
        )]);

        assert!(app.author_enrichment_rx.is_some());
    }

    #[test]
    fn state_change_enrichment_runs_with_cached_context() {
        let mut app = App::new(Config::default());
        app.set_api_client(GitHubClient::new_test());

        let mut notification = notification_with("1", "State changed", "state_change", None);
        notification.context = Some("closed".to_string());
        notification.subject.url = Some("http://127.0.0.1:9/repos/owner/repo/issues/1".to_string());

        app.merge_refreshed_notifications(vec![notification]);

        assert!(app.author_enrichment_rx.is_some());
    }

    #[test]
    fn mark_all_confirm_message_mentions_filtered_notifications() {
        let mut app = App::new(Config::default());
        app.base_filter = Some(Filter::from_pattern(Some("Alpha")).unwrap());
        app.base_filter_pattern = Some("Alpha".to_string());
        app.state.set_notifications(vec![
            notification_with("1", "Alpha", "mention", None),
            notification_with("2", "Beta", "mention", None),
        ]);
        app.state.set_filter(app.base_filter.clone());
        app.state
            .set_filter_pattern(app.base_filter_pattern.clone());

        app.execute_confirmed_action(ConfirmAction::MarkAllRead {
            selected: MarkAllOption::MarkReadOnly,
        })
        .unwrap();

        assert_eq!(
            app.state.loading_message,
            "Marking filtered notifications as read"
        );
    }

    #[test]
    fn open_selection_prints_pluralised_status_for_print_method() {
        let mut app = App::new(Config {
            open_method: OpenMethod::Print,
            ..Config::default()
        });
        app.state.set_notifications(vec![
            test_notification("1", true),
            test_notification("2", true),
        ]);
        app.state.toggle_selection("1".to_string());
        app.state.toggle_selection("2".to_string());

        app.handle_normal_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE))
            .unwrap();

        assert_eq!(
            app.state.status_message.as_deref(),
            Some("Printed 2 notifications")
        );
        assert_eq!(app.pending_print_urls.len(), 2);
    }

    #[test]
    fn enter_selection_pluralises_without_auto_mark_on_open() {
        let mut app = App::new(Config {
            open_method: OpenMethod::Print,
            auto_mark_on_open: false,
            ..Config::default()
        });
        app.state
            .set_notifications(vec![test_notification("1", true)]);
        app.state.toggle_selection("1".to_string());

        app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();

        assert_eq!(
            app.state.status_message.as_deref(),
            Some("Printed 1 notification")
        );
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Save pinned state if dirty
        if self.pinned_state_dirty {
            let pinned = self.state.get_pinned_notifications();
            if let Err(e) = crate::state_file::AppStateFile::save_pinned_notifications(pinned) {
                eprintln!(
                    "Warning: Failed to save pinned notifications on exit: {}",
                    e
                );
            }
        }

        self.preview_manager.take();
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;
    use crate::config::Action;
    use crate::models::{NotificationType, Owner, Repository, Subject};

    fn create_notification(id: &str, title: &str, number: u32) -> Notification {
        Notification {
            id: id.to_string(),
            unread: true,
            last_read_at: None,
            updated_at: None,
            reason: "mention".to_string(),
            repository: Repository {
                id: 1,
                name: "gh-news".to_string(),
                full_name: "chmouel/gh-news".to_string(),
                owner: Owner {
                    login: "chmouel".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: title.to_string(),
                subject_type: NotificationType::Issue,
                url: Some(format!(
                    "https://api.github.com/repos/chmouel/gh-news/issues/{}",
                    number
                )),
                latest_comment_url: None,
            },
            latest_comment_url: None,
            author: None,
            context: None,
        }
    }

    fn create_test_app(action: Action) -> App {
        let config = Config {
            actions: vec![action],
            ..Config::default()
        };
        let mut app = App::new(config);
        app.state.set_notifications(vec![
            create_notification("12345", "First notification", 42),
            create_notification("99999", "Second notification", 99),
        ]);
        app
    }

    fn custom_action_index() -> usize {
        builtin_actions::BuiltinAction::all().len()
    }

    #[test]
    fn test_action_menu_enter_preserves_command_output_mode() {
        let action = Action {
            name: "Show output".to_string(),
            command: "printf 'hello'".to_string(),
            priority: None,
            interactive: false,
            show_output: true,
        };
        let mut app = create_test_app(action);
        app.state.action_menu_index = custom_action_index();
        app.state.input_mode = InputMode::ActionMenu;

        app.handle_action_menu_key(KeyEvent::from(KeyCode::Enter))
            .unwrap();

        assert_eq!(app.state.input_mode, InputMode::CommandOutput);
        let output = app.state.command_output.as_ref().unwrap();
        assert_eq!(output.title, "Show output");
        assert_eq!(output.content, "hello");
    }

    #[test]
    fn test_show_output_runs_for_each_selected_notification() {
        let action = Action {
            name: "Show ids".to_string(),
            command: "printf {id}".to_string(),
            priority: None,
            interactive: false,
            show_output: true,
        };
        let mut app = create_test_app(action);
        app.state.action_menu_index = custom_action_index();
        app.state.toggle_selection("12345".to_string());
        app.state.toggle_selection("99999".to_string());

        app.execute_selected_action();

        let output = app.state.command_output.as_ref().unwrap();
        assert_eq!(app.state.input_mode, InputMode::CommandOutput);
        assert!(output.content.contains("First notification (12345)\n12345"));
        assert!(output
            .content
            .contains("Second notification (99999)\n99999"));
        assert!(!app.state.has_selection());
    }
}
