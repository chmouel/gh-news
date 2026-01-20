use crate::config::Config;
use crate::error::Result;
use crate::hooks;
use crate::models::Notification;
use crate::preview::{PreviewData, PreviewFetcher};
use crate::state::{AppState, ConfirmAction, InputMode, MarkAllOption, PaneFocus, PreviewMode};
use crate::terminal::Terminal;
use crate::ui::components::{confirm, help, list, loading, preview, status};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use parking_lot::Mutex;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Request to fetch a preview in the background
#[derive(Debug)]
struct FetchRequest {
    notification_id: String,
    notification: Notification,
    config: Config,
}

/// Result from background preview fetch
#[derive(Debug)]
struct FetchResult {
    notification_id: String,
}

/// Dwell time before auto-marking a notification as read (ms)
const AUTO_MARK_READ_DWELL_MS: u64 = 400;

pub struct App {
    state: AppState,
    config: Config,
    should_quit: bool,
    list_widget: list::ListWidget,
    preview_widget: preview::PreviewWidget,
    status_widget: status::StatusWidget,
    help_widget: help::HelpWidget,
    confirm_widget: confirm::ConfirmWidget,
    loading_widget: loading::LoadingWidget,
    api_client: Option<crate::api::GitHubClient>,
    last_refresh: Instant,
    refresh_args: Option<(bool, bool, Option<usize>)>, // (all, participating, max_notifications)
    preview_cache: Arc<Mutex<HashMap<String, PreviewData>>>, // Thread-safe cache
    preview_loading: Arc<Mutex<HashSet<String>>>,      // Track in-flight requests
    previous_notification_ids: HashSet<String>,        // Track notification IDs for new detection
    // Background preview fetcher
    prefetch_tx: Option<Sender<FetchRequest>>,
    prefetch_result_rx: Option<Receiver<FetchResult>>,
    prefetch_thread: Option<JoinHandle<()>>,
    // Auto-mark-read state
    auto_mark_read_enabled: bool,
    pending_mark_read: Option<(String, Instant)>, // (notification_id, timestamp)
    // Track if pinned notifications need to be saved
    pinned_state_dirty: bool,
}

/// Background worker thread that fetches previews
fn preview_worker_thread(
    rx: Receiver<FetchRequest>,
    tx: Sender<FetchResult>,
    cache: Arc<Mutex<HashMap<String, PreviewData>>>,
    loading: Arc<Mutex<HashSet<String>>>,
) {
    while let Ok(request) = rx.recv() {
        // Check cache first (avoid redundant work)
        {
            let cache_lock = cache.lock();
            if cache_lock.contains_key(&request.notification_id) {
                loading.lock().remove(&request.notification_id);
                continue;
            }
        }

        // Create temporary client for this request
        let client_result = crate::api::GitHubClient::new(&request.config);
        let client = match client_result {
            Ok(c) => c,
            Err(e) => {
                // Failed to create client, cache error
                let error_data = PreviewData::Generic {
                    title: request.notification.title().to_string(),
                    notification_type: request.notification.notification_type(),
                    body: format!("Error creating API client\n\n{}", e),
                };
                let mut cache_lock = cache.lock();
                cache_lock.insert(request.notification_id.clone(), error_data);
                loading.lock().remove(&request.notification_id);
                continue;
            }
        };

        // Fetch preview
        let result = PreviewFetcher::fetch_preview(&client, &request.notification)
            .map_err(|e| e.to_string());

        // Handle result
        match &result {
            Ok(data) => {
                let mut cache_lock = cache.lock();
                cache_lock.insert(request.notification_id.clone(), data.clone());
            }
            Err(error) => {
                // Cache error to avoid retries
                let error_data = PreviewData::Generic {
                    title: request.notification.title().to_string(),
                    notification_type: request.notification.notification_type(),
                    body: format!("Error loading details\n\n{}", error),
                };
                let mut cache_lock = cache.lock();
                cache_lock.insert(request.notification_id.clone(), error_data);
            }
        }

        // Mark as no longer loading
        loading.lock().remove(&request.notification_id);

        // Send completion signal
        let _ = tx.send(FetchResult {
            notification_id: request.notification_id,
        });
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        let auto_mark_read = config.auto_mark_read;
        Self {
            state: AppState::new(),
            config,
            should_quit: false,
            list_widget: list::ListWidget::new(),
            preview_widget: preview::PreviewWidget::new(),
            status_widget: status::StatusWidget::new(),
            help_widget: help::HelpWidget::new(),
            confirm_widget: confirm::ConfirmWidget::new(),
            loading_widget: loading::LoadingWidget::new(),
            api_client: None,
            last_refresh: Instant::now(),
            refresh_args: None,
            preview_cache: Arc::new(Mutex::new(HashMap::new())),
            preview_loading: Arc::new(Mutex::new(HashSet::new())),
            previous_notification_ids: HashSet::new(),
            prefetch_tx: None,
            prefetch_result_rx: None,
            prefetch_thread: None,
            auto_mark_read_enabled: auto_mark_read,
            pending_mark_read: None,
            pinned_state_dirty: false,
        }
    }

    pub fn set_auto_mark_read(&mut self, enabled: bool) {
        self.auto_mark_read_enabled = enabled;
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

                // Call API (non-blocking, log errors)
                if let Some(ref client) = self.api_client {
                    if let Err(e) = client.mark_notification_read(notification_id) {
                        eprintln!("Failed to auto-mark notification as read: {}", e);
                    }
                }

                self.pending_mark_read = None;
            }
        }
    }

    pub fn set_api_client(&mut self, client: crate::api::GitHubClient) {
        self.api_client = Some(client);
    }

    /// Start the background preview worker thread
    pub fn start_preview_worker(&mut self) {
        let (req_tx, req_rx) = channel::<FetchRequest>();
        let (result_tx, result_rx) = channel::<FetchResult>();
        let cache = Arc::clone(&self.preview_cache);
        let loading = Arc::clone(&self.preview_loading);

        let handle = thread::spawn(move || {
            preview_worker_thread(req_rx, result_tx, cache, loading);
        });

        self.prefetch_tx = Some(req_tx);
        self.prefetch_result_rx = Some(result_rx);
        self.prefetch_thread = Some(handle);
    }

    /// Open a URL in the browser using custom command if configured, otherwise system default.
    fn open_url(&self, url: &str) -> std::io::Result<()> {
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

    pub fn start_auto_refresh(
        &mut self,
        all: bool,
        participating: bool,
        max_notifications: Option<usize>,
    ) {
        // Always store refresh args for manual refresh (Ctrl+R)
        self.refresh_args = Some((all, participating, max_notifications));

        if self.config.auto_refresh_interval == 0 {
            return; // Auto-refresh disabled, but manual refresh still works
        }

        let _interval = self.config.auto_refresh_interval;

        // Store refresh args - we'll check the timer in the main loop
        // No need for a separate thread since we're already polling events
    }

    fn refresh_notifications(&mut self) -> Result<()> {
        if let Some(ref client) = self.api_client {
            if let Some((all, participating, max_notifications)) = self.refresh_args {
                // Don't show loading indicator for auto-refresh to avoid UI flicker
                // self.state.loading = true;
                // self.state.loading_message = "Refreshing notifications...".to_string();

                // Fetch notifications
                let mut all_notifications = Vec::new();
                let mut page = 1;
                let per_page = 50.min(max_notifications.unwrap_or(usize::MAX));

                loop {
                    let notifications =
                        client.get_notifications(all, participating, Some(per_page), Some(page))?;

                    if notifications.is_empty() {
                        break;
                    }

                    let remaining = max_notifications
                        .unwrap_or(usize::MAX)
                        .saturating_sub(all_notifications.len());
                    if remaining == 0 {
                        break;
                    }

                    let to_take = remaining.min(notifications.len());
                    all_notifications.extend(notifications.into_iter().take(to_take));

                    if all_notifications.len() >= max_notifications.unwrap_or(usize::MAX) {
                        break;
                    }

                    page += 1;
                }

                // Preserve current selection if possible
                let old_selected = self.state.selected_index;
                let old_notif_id = self
                    .state
                    .filtered_notifications
                    .get(old_selected)
                    .and_then(|&idx| self.state.notifications.get(idx))
                    .map(|n| n.id.clone());

                // Preserve current filter
                let current_filter = self.state.filter.clone();
                let filter_pattern = self.state.filter_pattern.clone();

                self.state.set_notifications(all_notifications);

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
                    if let Some(new_idx) =
                        self.state.notifications.iter().position(|n| n.id == old_id)
                    {
                        if let Some(filtered_idx) = self
                            .state
                            .filtered_notifications
                            .iter()
                            .position(|&idx| idx == new_idx)
                        {
                            self.state.selected_index = filtered_idx;
                        }
                    }
                }

                // Ensure selected_index is valid
                if !self.state.filtered_notifications.is_empty() {
                    self.state.selected_index = self
                        .state
                        .selected_index
                        .min(self.state.filtered_notifications.len() - 1);
                } else {
                    self.state.selected_index = 0;
                }

                // Auto-fetch preview for selected notification after refresh
                if self.state.show_preview() {
                    self.auto_fetch_preview_for_selected();
                }

                self.state.loading = false;
                self.last_refresh = Instant::now();
            }
        }
        Ok(())
    }

    pub fn update_state(&mut self, state: AppState) {
        self.state = state;
        // Auto-fetch preview for first notification if preview is enabled
        self.auto_fetch_preview_for_selected();
    }

    fn auto_fetch_preview_for_selected(&mut self) {
        // Only auto-fetch if preview is enabled and there's no content yet
        // Used for initial load
        if !self.state.show_preview() || self.state.preview_content.is_some() {
            return;
        }

        self.fetch_preview_for_selected_notification();
        self.prefetch_next_preview();
    }

    fn fetch_preview_for_selected_notification(&mut self) {
        // Get selected notification and clone data we need
        let (notification_id, notification_type, notification_clone) =
            match self.state.selected_notification() {
                Some(n) => (n.id.clone(), n.notification_type(), n.clone()),
                None => {
                    self.state.preview_content = None;
                    return;
                }
            };

        // Check cache first
        {
            let cache = self.preview_cache.lock();
            if let Some(cached) = cache.get(&notification_id) {
                self.state.preview_content = Some(cached.clone());
                self.state.preview_scroll = 0;
                return;
            }
        }

        // Check if already loading
        {
            let loading = self.preview_loading.lock();
            if loading.contains(&notification_id) {
                // Already being fetched, show loading state
                self.state.preview_content = Some(PreviewData::Generic {
                    title: "Loading details...".to_string(),
                    notification_type,
                    body: "⏳ Fetching details...\n\nThis may take a moment.".to_string(),
                });
                return;
            }
        }

        // Mark as loading
        self.preview_loading.lock().insert(notification_id.clone());

        // Show loading state immediately
        self.state.preview_content = Some(PreviewData::Generic {
            title: "Loading details...".to_string(),
            notification_type,
            body: "⏳ Fetching details...\n\nThis may take a moment.".to_string(),
        });
        self.state.preview_scroll = 0;

        // Send to background thread
        if let Some(ref tx) = self.prefetch_tx {
            let _ = tx.send(FetchRequest {
                notification_id,
                notification: notification_clone,
                config: self.config.clone(),
            });
        }
    }

    pub fn fetch_preview_for_selected(&mut self) {
        self.auto_fetch_preview_for_selected();
    }

    /// Prefetch the next notification's details in the background
    fn prefetch_next_preview(&mut self) {
        // Don't prefetch if preview is disabled
        if !self.state.show_preview() {
            return;
        }

        // Find next notification in tree
        let current_idx = self.state.selected_index;

        // Search forward from current position for next Notification item
        let next_notif = (current_idx + 1..self.state.tree_items.len()).find_map(|i| {
            if let Some(crate::state::TreeItem::Notification(notif_idx)) =
                self.state.tree_items.get(i)
            {
                self.state.notifications.get(*notif_idx)
            } else {
                None
            }
        });

        // If found, queue for prefetch
        if let Some(notif) = next_notif {
            let notification_id = notif.id.clone();

            // Skip if already cached or currently loading
            {
                let cache = self.preview_cache.lock();
                let loading = self.preview_loading.lock();
                if cache.contains_key(&notification_id) || loading.contains(&notification_id) {
                    return; // Already have it or it's being fetched
                }
            }

            // Mark as loading and send fetch request
            self.preview_loading.lock().insert(notification_id.clone());

            if let Some(ref tx) = self.prefetch_tx {
                let _ = tx.send(FetchRequest {
                    notification_id,
                    notification: notif.clone(),
                    config: self.config.clone(),
                });
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

            // Check for auto-refresh signal (non-blocking)
            if self.config.auto_refresh_interval > 0 {
                let elapsed = self.last_refresh.elapsed();
                if elapsed >= Duration::from_secs(self.config.auto_refresh_interval) {
                    if let Err(e) = self.refresh_notifications() {
                        // Log error but don't disrupt UI - user can manually refresh
                        // In production, this could be sent to a logging system
                        // For now, we silently continue to avoid UI disruption
                        let _ = e; // Acknowledge error but don't panic
                    }
                }
            }

            // Poll for background fetch completions (non-blocking)
            if let Some(ref rx) = self.prefetch_result_rx {
                while let Ok(result) = rx.try_recv() {
                    // Check if this is for the currently selected notification
                    if let Some(current_notif) = self.state.selected_notification() {
                        if current_notif.id == result.notification_id {
                            // Update preview with fetched data
                            let cache = self.preview_cache.lock();
                            if let Some(data) = cache.get(&result.notification_id) {
                                self.state.preview_content = Some(data.clone());
                                self.state.preview_scroll = 0;
                            }
                        }
                    }
                }
            }

            // Process pending auto-mark-read (debounced)
            self.process_pending_mark_read();

            // Use standard timeout for native ratatui behavior
            let timeout = Duration::from_millis(250); // Standard ratatui polling timeout

            if event::poll(timeout).map_err(|e| crate::error::Error::Terminal(e.to_string()))? {
                match event::read().map_err(|e| crate::error::Error::Terminal(e.to_string()))? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            self.handle_key(key)?;
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
            self.help_widget.render(frame, size);
            return;
        }

        if self.state.loading {
            self.loading_widget
                .render(frame, size, &self.state.loading_message);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(0),    // Main content
            ])
            .split(size);

        self.status_widget
            .render(frame, chunks[0], &self.state, &self.config);

        // Check if a pane is focused (zoomed)
        match self.state.focused_pane {
            PaneFocus::Pane1 => {
                // Show only list widget in full screen
                self.list_widget.render(frame, chunks[1], &self.state);
            }
            PaneFocus::Pane2 => {
                // Show only preview widget in full screen (if preview is enabled)
                if self.state.show_preview() {
                    self.preview_widget.render(frame, chunks[1], &self.state);
                } else {
                    // If preview is off but pane 2 is focused, show list instead
                    self.list_widget.render(frame, chunks[1], &self.state);
                }
            }
            PaneFocus::None => {
                // Normal split layout based on preview_mode
                let main_chunks = match self.state.preview_mode {
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

                match self.state.preview_mode {
                    PreviewMode::Off => {
                        self.list_widget.render(frame, main_chunks[0], &self.state);
                    }
                    PreviewMode::Horizontal => {
                        self.list_widget.render(frame, main_chunks[0], &self.state);
                        self.preview_widget
                            .render(frame, main_chunks[1], &self.state);
                    }
                    PreviewMode::Vertical => {
                        self.list_widget.render(frame, main_chunks[0], &self.state);
                        self.preview_widget
                            .render(frame, main_chunks[1], &self.state);
                    }
                }
            }
        }

        // Render confirmation dialog as overlay (if active)
        if self.state.input_mode == InputMode::Confirm {
            if let Some(ConfirmAction::MarkAllRead { selected }) = self.state.confirm_action {
                self.confirm_widget.render(frame, size, selected);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.state.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Search => self.handle_search_key(key),
            InputMode::Help => {
                if key.code == KeyCode::Char('?')
                    || key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Esc
                {
                    self.state.show_help = false;
                    self.state.input_mode = InputMode::Normal;
                }
                Ok(())
            }
            InputMode::Confirm => self.handle_confirm_key(key),
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
                self.state.confirm_action = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ConfirmAction::MarkAllRead { ref mut selected }) =
                    self.state.confirm_action
                {
                    *selected = MarkAllOption::MarkReadAndArchive;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ConfirmAction::MarkAllRead { ref mut selected }) =
                    self.state.confirm_action
                {
                    *selected = MarkAllOption::MarkReadOnly;
                }
            }
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

    fn execute_confirmed_action(&mut self, action: ConfirmAction) -> Result<()> {
        match action {
            ConfirmAction::MarkAllRead { selected } => {
                if let Some(ref client) = self.api_client {
                    match selected {
                        MarkAllOption::MarkReadAndArchive => {
                            // Archive (mark as done) each notification individually
                            for notif in &self.state.notifications {
                                let _ = client.mark_thread_done(&notif.id);
                            }
                        }
                        MarkAllOption::MarkReadOnly => {
                            // Just mark all as read
                            client.mark_all_read(None)?;
                        }
                    }
                    // Update local state
                    for notif in &mut self.state.notifications {
                        notif.unread = false;
                    }
                    // Refresh to sync with server
                    self.refresh_notifications()?;
                }
            }
        }
        Ok(())
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // If a pane is focused, zoom out to split view
                if self.state.focused_pane != PaneFocus::None {
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
                self.state.show_help = true;
                self.state.input_mode = InputMode::Help;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_up();
                // Scroll preview to top when selection changes
                self.state.preview_scroll = 0;
                // Auto-fetch preview for the newly selected notification
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_next_preview();
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
                    self.prefetch_next_preview();
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
                        self.prefetch_next_preview();
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
                        self.prefetch_next_preview();
                    }
                    self.queue_auto_mark_read();
                }
            }
            KeyCode::Home => {
                self.state.selected_index = 0;
                self.state.preview_scroll = 0;
                if self.state.show_preview() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_next_preview();
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
                    self.prefetch_next_preview();
                }
                self.queue_auto_mark_read();
            }
            KeyCode::Enter => {
                // If selected item is a repository header, expand it and select first notification
                if let Some(repo_name) = self.state.selected_repo() {
                    let repo_name = repo_name.to_string();

                    // Expand the repo if it's collapsed
                    let was_expanded = self
                        .state
                        .expanded_repos
                        .get(&repo_name)
                        .copied()
                        .unwrap_or(true);
                    if !was_expanded {
                        self.state.toggle_repo_expansion(&repo_name);
                    }

                    // After expansion, tree_items is rebuilt, so find the first notification now
                    if let Some(first_notif_idx) = self
                        .state
                        .tree_items
                        .iter()
                        .enumerate()
                        .find_map(|(idx, item)| {
                            if let crate::state::TreeItem::Notification(notif_idx) = item {
                                if let Some(notif) = self.state.notifications.get(*notif_idx) {
                                    if notif.repo_full_name() == repo_name {
                                        return Some(idx);
                                    }
                                }
                            }
                            None
                        })
                    {
                        // Select the first notification
                        self.state.selected_index = first_notif_idx;
                        self.state.preview_scroll = 0;

                        // Ensure preview is shown
                        if self.state.preview_mode == PreviewMode::Off {
                            self.state.preview_mode = PreviewMode::Horizontal;
                        }

                        // Fetch preview for the selected notification
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_next_preview();
                    }
                } else if let Some(notification) = self.state.selected_notification() {
                    // Open the notification URL in the browser
                    if let Some(url) = notification.web_url() {
                        if let Err(e) = self.open_url(&url) {
                            eprintln!("Failed to open URL {}: {}", url, e);
                        }
                    } else {
                        eprintln!("No URL available for this notification");
                    }

                    // Mark notification as read if it's unread
                    if notification.is_unread() {
                        let notification_id = notification.id.clone();

                        // Update local state optimistically (for better UX)
                        self.state.mark_notification_read(&notification_id);

                        // Call API to mark as read (non-blocking, log errors)
                        if let Some(ref client) = self.api_client {
                            if let Err(e) = client.mark_notification_read(&notification_id) {
                                eprintln!("Failed to mark notification as read: {}", e);
                                // Note: We've already updated local state optimistically,
                                // so the UI will show it as read even if API call fails
                            }
                        }
                    }
                }
            }
            KeyCode::Char('o') => {
                // Open notification URL without marking as read
                if let Some(notification) = self.state.selected_notification() {
                    if let Some(url) = notification.web_url() {
                        if let Err(e) = self.open_url(&url) {
                            eprintln!("Failed to open URL {}: {}", url, e);
                        }
                    } else {
                        eprintln!("No URL available for this notification");
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Space bar toggles repository expansion
                if let Some(repo_name) = self.state.selected_repo() {
                    let repo_name = repo_name.to_string();
                    self.state.toggle_repo_expansion(&repo_name);
                }
            }
            KeyCode::Char('h') => {
                // Collapse the current folder if expanded
                if let Some(repo_name) = self.state.parent_repo_for_selected() {
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
            KeyCode::Char('t') => {
                // Toggle read/unread status of selected notification
                if let Some(notification) = self.state.selected_notification() {
                    let notification_id = notification.id.clone();
                    let was_unread = notification.is_unread();

                    // Toggle local state
                    if let Some(is_now_unread) =
                        self.state.toggle_notification_read(&notification_id)
                    {
                        // If marking as read (was unread, now read), call API
                        if was_unread && !is_now_unread {
                            if let Some(ref client) = self.api_client {
                                if let Err(e) = client.mark_notification_read(&notification_id) {
                                    eprintln!("Failed to mark notification as read: {}", e);
                                    // Revert local state on API failure
                                    self.state.toggle_notification_read(&notification_id);
                                }
                            }
                        }
                        // If marking as unread (was read, now unread), just update local state
                        // Note: GitHub API doesn't support marking as unread, so this won't persist on refresh
                    }
                }
            }
            KeyCode::Char('!') => {
                // Toggle pin status of selected notification
                if let Some(notification) = self.state.selected_notification() {
                    // Capture notification ID before rebuild to preserve selection
                    let selected_notif_id = notification.id.clone();
                    let notification = notification.clone();

                    self.state.toggle_pin(notification);

                    // Mark state as dirty instead of immediate save
                    self.pinned_state_dirty = true;

                    // Rebuild tree to move notification to/from pinned section
                    self.state.build_tree();

                    // Restore selection to same notification
                    if let Some(new_idx) = self.state.tree_items.iter().position(|item| {
                        if let crate::state::TreeItem::Notification(notif_idx) = item {
                            self.state
                                .notifications
                                .get(*notif_idx)
                                .map(|n| n.id == selected_notif_id)
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    }) {
                        self.state.selected_index = new_idx;
                    } else {
                        // Fallback: select first notification if original not found
                        if let Some(first_notif) = self.state.tree_items.iter().position(|item| {
                            matches!(item, crate::state::TreeItem::Notification(_))
                        }) {
                            self.state.selected_index = first_notif;
                        }
                    }

                    // Refresh preview for selected notification
                    if self.state.show_preview() {
                        self.fetch_preview_for_selected_notification();
                        self.prefetch_next_preview();
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

                // Save the new preview mode to state file
                if let Err(e) = crate::state_file::AppStateFile::save(self.state.preview_mode) {
                    // Log error but don't disrupt UI - state saving is best effort
                    let _ = e;
                }

                // If showing preview and no content loaded, fetch it (uses cache)
                if self.state.show_preview() && self.state.preview_content.is_none() {
                    self.fetch_preview_for_selected_notification();
                    self.prefetch_next_preview();
                }
            }
            KeyCode::Char('M') => {
                // Toggle auto-mark-read on scroll
                self.auto_mark_read_enabled = !self.auto_mark_read_enabled;

                // Clear pending mark if disabling
                if !self.auto_mark_read_enabled {
                    self.pending_mark_read = None;
                }

                // Save to state file
                if let Err(e) = crate::state_file::AppStateFile::save_auto_mark_read(
                    self.auto_mark_read_enabled,
                ) {
                    let _ = e;
                }
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
                self.refresh_notifications()?;
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Show confirmation dialog for mark all as read
                if !self.state.notifications.is_empty() {
                    self.state.confirm_action = Some(ConfirmAction::MarkAllRead {
                        selected: MarkAllOption::MarkReadOnly, // Default
                    });
                    self.state.input_mode = InputMode::Confirm;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state.input_mode = InputMode::Normal;
                self.state.search_query.clear();
            }
            KeyCode::Enter => {
                self.state.input_mode = InputMode::Normal;
                // Apply search filter
            }
            KeyCode::Char(c) => {
                self.state.search_query.push(c);
            }
            KeyCode::Backspace => {
                self.state.search_query.pop();
            }
            _ => {}
        }
        Ok(())
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
                match self.state.preview_mode {
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
                match self.state.preview_mode {
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
                                self.prefetch_next_preview();
                            } else {
                                // Enable preview if it's off
                                self.state.preview_mode = PreviewMode::Horizontal;
                                self.fetch_preview_for_selected_notification();
                                self.prefetch_next_preview();
                            }
                            self.queue_auto_mark_read();
                        } else if let Some(crate::state::TreeItem::RepositoryHeader(repo_name)) =
                            self.state.tree_items.get(clicked_item_idx)
                        {
                            // Clicked on a repository header - toggle expansion
                            let repo_name = repo_name.clone();
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
                                    self.prefetch_next_preview();
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
                                self.prefetch_next_preview();
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

        // Signal worker to exit by dropping the channel
        drop(self.prefetch_tx.take());

        // Wait for worker thread to finish
        if let Some(handle) = self.prefetch_thread.take() {
            let _ = handle.join();
        }
    }
}
