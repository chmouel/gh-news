use crate::filter::Filter;
use crate::models::Notification;
use crate::preview::PreviewData;
use std::collections::{HashMap, HashSet};

mod tree;

use tree::TreeBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeItem {
    PinnedHeader,             // Header for pinned notifications section
    RepositoryHeader(String), // Repository full name
    Notification(usize),      // Index into notifications
}

pub struct AppState {
    pub notifications: Vec<Notification>,
    pub filtered_notifications: Vec<usize>, // Indices into notifications
    pub tree_items: Vec<TreeItem>,          // Tree structure for display
    pub expanded_repos: HashMap<String, bool>, // Track which repos are expanded
    pub selected_index: usize,
    pub filter: Option<Filter>,
    pub preview_mode: PreviewMode,
    pub focused_pane: PaneFocus,
    pub show_help: bool,
    pub preview_content: Option<PreviewData>,
    pub preview_scroll: usize,
    pub help_scroll: usize,
    pub help_search_query: String,
    pub help_view_height: usize,
    pub help_content_len: usize,
    pub input_mode: InputMode,
    pub search_query: String,
    pub loading: bool,
    pub loading_message: String,
    // Store original filter pattern for filter management
    pub filter_pattern: Option<String>,
    // Track if we're showing all notifications (read and unread)
    pub show_all: bool,
    // Confirmation dialog state
    pub confirm_action: Option<ConfirmAction>,
    // Pinned notifications (stored as full data to persist after being marked read)
    pub pinned_notifications: Vec<Notification>,
    // Status message displayed in the status bar (e.g., after marking all as read)
    pub status_message: Option<String>,
    // Multi-select: notification IDs that are currently selected
    pub selected_notification_ids: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InputMode {
    Normal,
    Help,
    HelpSearch,
    Search,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Off,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    None,  // Both panes visible (split view)
    Pane1, // Notifications list zoomed
    Pane2, // Preview zoomed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAllOption {
    MarkReadOnly,       // Default - just mark as read
    MarkReadAndArchive, // Mark as read AND archive (done)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    MarkAllRead { selected: MarkAllOption },
    ArchiveSelected { count: usize, option: MarkAllOption },
}

impl AppState {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            filtered_notifications: Vec::new(),
            tree_items: Vec::new(),
            expanded_repos: HashMap::new(),
            selected_index: 0,
            filter: None,
            preview_mode: PreviewMode::Vertical, // Preview open by default
            focused_pane: PaneFocus::None,
            show_help: false,
            preview_content: None,
            preview_scroll: 0,
            help_scroll: 0,
            help_search_query: String::new(),
            help_view_height: 0,
            help_content_len: 0,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            loading: false,
            loading_message: String::new(),
            filter_pattern: None,
            show_all: false,
            confirm_action: None,
            pinned_notifications: Vec::new(),
            status_message: None,
            selected_notification_ids: HashSet::new(),
        }
    }

    pub fn toggle_pin(&mut self, notification: Notification) -> bool {
        if let Some(pos) = self
            .pinned_notifications
            .iter()
            .position(|n| n.id == notification.id)
        {
            self.pinned_notifications.remove(pos);
            false
        } else {
            self.pinned_notifications.push(notification);
            true
        }
    }

    pub fn is_pinned(&self, notification_id: &str) -> bool {
        self.pinned_notifications
            .iter()
            .any(|n| n.id == notification_id)
    }

    pub fn set_pinned_notifications(&mut self, notifications: Vec<Notification>) {
        self.pinned_notifications = notifications;
    }

    pub fn get_pinned_notifications(&self) -> &[Notification] {
        &self.pinned_notifications
    }

    pub fn set_notifications(&mut self, notifications: Vec<Notification>) {
        self.notifications = notifications;
        self.apply_filter();
    }

    pub fn set_filter(&mut self, filter: Option<Filter>) {
        self.filter = filter;
        self.apply_filter();
    }

    pub fn set_filter_pattern(&mut self, pattern: Option<String>) {
        self.filter_pattern = pattern;
    }

    fn apply_filter(&mut self) {
        if let Some(ref filter) = self.filter {
            self.filtered_notifications = self
                .notifications
                .iter()
                .enumerate()
                .filter_map(|(i, n)| if filter.matches(n) { Some(i) } else { None })
                .collect();
        } else {
            self.filtered_notifications = (0..self.notifications.len()).collect();
        }

        // Build tree structure grouped by repository
        self.build_tree();

        self.select_first_notification();
    }

    pub fn build_tree(&mut self) {
        let pinned_ids: HashSet<String> = self
            .pinned_notifications
            .iter()
            .map(|n| n.id.clone())
            .collect();

        self.tree_items = TreeBuilder::new(
            &self.notifications,
            &self.filtered_notifications,
            &self.expanded_repos,
            &pinned_ids,
        )
        .build();
    }

    pub fn toggle_repo_expansion(&mut self, repo_name: &str) {
        let current = self.expanded_repos.get(repo_name).copied().unwrap_or(true);
        self.expanded_repos.insert(repo_name.to_string(), !current);
        self.build_tree();

        // Adjust selected_index if needed
        if !self.tree_items.is_empty() {
            self.selected_index = self.selected_index.min(self.tree_items.len() - 1);
        }
    }

    /// Collapse all repositories (set all to collapsed state).
    pub fn collapse_all_repos(&mut self) {
        // Get all unique repo names from notifications
        let repo_names: Vec<String> = self
            .notifications
            .iter()
            .map(|n| n.repo_full_name().to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for repo_name in repo_names {
            self.expanded_repos.insert(repo_name, false);
        }
    }

    pub fn selected_notification(&self) -> Option<&Notification> {
        self.tree_items
            .get(self.selected_index)
            .and_then(|item| match item {
                TreeItem::Notification(idx) => self.notifications.get(*idx),
                TreeItem::RepositoryHeader(_) | TreeItem::PinnedHeader => None,
            })
    }

    pub fn selected_repo(&self) -> Option<&str> {
        self.tree_items
            .get(self.selected_index)
            .and_then(|item| match item {
                TreeItem::RepositoryHeader(repo) => Some(repo.as_str()),
                TreeItem::Notification(_) | TreeItem::PinnedHeader => None,
            })
    }

    pub fn parent_repo_for_selected(&self) -> Option<&str> {
        // If the selected item is a RepositoryHeader, return it directly
        if let Some(TreeItem::RepositoryHeader(repo)) = self.tree_items.get(self.selected_index) {
            return Some(repo.as_str());
        }

        if let Some(TreeItem::Notification(idx)) = self.tree_items.get(self.selected_index) {
            return self
                .notifications
                .get(*idx)
                .map(|notification| notification.repo_full_name());
        }

        // Fallback for headers without a direct notification (keeps behaviour for legacy layouts).
        for i in (0..self.selected_index).rev() {
            if let Some(TreeItem::RepositoryHeader(repo)) = self.tree_items.get(i) {
                return Some(repo.as_str());
            }
        }

        None
    }

    pub fn select_first_notification(&mut self) -> bool {
        if let Some(first_notif_idx) = self
            .tree_items
            .iter()
            .position(|item| matches!(item, TreeItem::Notification(_)))
        {
            self.selected_index = first_notif_idx;
            true
        } else {
            if !self.tree_items.is_empty() {
                self.selected_index = self.selected_index.min(self.tree_items.len() - 1);
            } else {
                self.selected_index = 0;
            }
            false
        }
    }

    pub fn select_notification_by_id(&mut self, notification_id: &str) -> bool {
        if let Some(new_idx) = self.tree_items.iter().position(|item| {
            if let TreeItem::Notification(notif_idx) = item {
                self.notifications
                    .get(*notif_idx)
                    .map(|notification| notification.id == notification_id)
                    .unwrap_or(false)
            } else {
                false
            }
        }) {
            self.selected_index = new_idx;
            true
        } else {
            false
        }
    }

    pub fn move_up(&mut self) {
        if self.tree_items.is_empty() {
            return;
        }

        // Early return if already at the first item
        if self.selected_index == 0 {
            return;
        }

        // If we're on a header, move to the previous visible item.
        if matches!(
            self.tree_items.get(self.selected_index),
            Some(TreeItem::RepositoryHeader(_) | TreeItem::PinnedHeader)
        ) {
            self.selected_index -= 1;
            return;
        }

        // Find the previous notification, skipping folders
        for i in (0..self.selected_index).rev() {
            if matches!(self.tree_items.get(i), Some(TreeItem::Notification(_))) {
                self.selected_index = i;
                return;
            }
        }
        // If no previous notification found, move to the previous visible item.
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.tree_items.is_empty() {
            return;
        }

        // Early return if already at the last item
        if self.selected_index >= self.tree_items.len().saturating_sub(1) {
            return;
        }

        // If we're on a header, move to the next visible item.
        if matches!(
            self.tree_items.get(self.selected_index),
            Some(TreeItem::RepositoryHeader(_) | TreeItem::PinnedHeader)
        ) {
            self.selected_index += 1;
            return;
        }

        // Find the next notification, skipping folders
        for i in (self.selected_index + 1)..self.tree_items.len() {
            if matches!(self.tree_items.get(i), Some(TreeItem::Notification(_))) {
                self.selected_index = i;
                return;
            }
        }
        // If no next notification found, move to the next visible item.
        if self.selected_index + 1 < self.tree_items.len() {
            self.selected_index += 1;
        }
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_notifications.len()
    }

    pub fn show_preview(&self) -> bool {
        self.preview_mode != PreviewMode::Off && !self.notifications.is_empty()
    }

    pub fn mark_notification_read(&mut self, notification_id: &str) {
        if let Some(notif) = self
            .notifications
            .iter_mut()
            .find(|n| n.id == notification_id)
        {
            notif.unread = false;
        }
    }

    pub fn toggle_notification_read(&mut self, notification_id: &str) -> Option<bool> {
        if let Some(notif) = self
            .notifications
            .iter_mut()
            .find(|n| n.id == notification_id)
        {
            notif.unread = !notif.unread;
            Some(notif.unread)
        } else {
            None
        }
    }

    // Multi-select helper methods

    pub fn is_selected(&self, notification_id: &str) -> bool {
        self.selected_notification_ids.contains(notification_id)
    }

    pub fn toggle_selection(&mut self, notification_id: String) {
        if self.selected_notification_ids.contains(&notification_id) {
            self.selected_notification_ids.remove(&notification_id);
        } else {
            self.selected_notification_ids.insert(notification_id);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_notification_ids.clear();
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_notification_ids.is_empty()
    }

    pub fn selection_count(&self) -> usize {
        self.selected_notification_ids.len()
    }

    pub fn get_selected_notification_ids(&self) -> Vec<String> {
        self.selected_notification_ids.iter().cloned().collect()
    }

    pub fn select_all_in_repo(&mut self, repo_name: &str) {
        for notification in &self.notifications {
            if notification.repo_full_name() == repo_name {
                self.selected_notification_ids
                    .insert(notification.id.clone());
            }
        }
    }
}
