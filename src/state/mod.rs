use crate::filter::Filter;
use crate::models::Notification;
use crate::preview::PreviewData;
use std::collections::{HashMap, HashSet};

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
    // Pinned notification IDs
    pub pinned_notification_ids: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InputMode {
    Normal,
    Help,
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
            input_mode: InputMode::Normal,
            search_query: String::new(),
            loading: false,
            loading_message: String::new(),
            filter_pattern: None,
            show_all: false,
            confirm_action: None,
            pinned_notification_ids: HashSet::new(),
        }
    }

    pub fn toggle_pin(&mut self, notification_id: &str) -> bool {
        if self.pinned_notification_ids.contains(notification_id) {
            self.pinned_notification_ids.remove(notification_id);
            false
        } else {
            self.pinned_notification_ids
                .insert(notification_id.to_string());
            true
        }
    }

    pub fn is_pinned(&self, notification_id: &str) -> bool {
        self.pinned_notification_ids.contains(notification_id)
    }

    pub fn set_pinned_notifications(&mut self, ids: Vec<String>) {
        self.pinned_notification_ids = ids.into_iter().collect();
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

        // Find the first notification (not a repository header) and select it
        if let Some(first_notif_idx) = self.tree_items.iter().enumerate().find_map(|(idx, item)| {
            if matches!(item, TreeItem::Notification(_)) {
                Some(idx)
            } else {
                None
            }
        }) {
            self.selected_index = first_notif_idx;
        } else {
            // No notifications available, just ensure selected_index is valid
            if !self.tree_items.is_empty() {
                self.selected_index = self.selected_index.min(self.tree_items.len() - 1);
            } else {
                self.selected_index = 0;
            }
        }
    }

    pub fn build_tree(&mut self) {
        use chrono::{DateTime, Utc};

        self.tree_items.clear();

        // Partition into pinned and non-pinned notifications
        let (pinned_indices, regular_indices): (Vec<usize>, Vec<usize>) =
            self.filtered_notifications.iter().partition(|&&idx| {
                self.notifications
                    .get(idx)
                    .map(|n| self.pinned_notification_ids.contains(&n.id))
                    .unwrap_or(false)
            });

        // Add pinned section if there are pinned notifications
        if !pinned_indices.is_empty() {
            self.tree_items.push(TreeItem::PinnedHeader);

            // Sort pinned notifications by timestamp (newest first)
            let mut sorted_pinned = pinned_indices;
            sorted_pinned.sort_by(|&a, &b| {
                let timestamp_a = self
                    .notifications
                    .get(a)
                    .and_then(|n| n.effective_timestamp())
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);
                let timestamp_b = self
                    .notifications
                    .get(b)
                    .and_then(|n| n.effective_timestamp())
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);
                timestamp_b.cmp(&timestamp_a)
            });

            for idx in sorted_pinned {
                self.tree_items.push(TreeItem::Notification(idx));
            }
        }

        // Group non-pinned notifications by repository
        let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();

        for idx in regular_indices {
            if let Some(notif) = self.notifications.get(idx) {
                let repo_name = notif.repo_full_name().to_string();
                repo_groups.entry(repo_name).or_default().push(idx);
            }
        }

        // Create a vector of (repo_name, notif_indices, latest_timestamp) for sorting
        let mut repo_list: Vec<(String, Vec<usize>, DateTime<Utc>)> = repo_groups
            .into_iter()
            .map(|(repo_name, mut notif_indices)| {
                // Sort notifications within this repository by timestamp (newest first)
                notif_indices.sort_by(|&a, &b| {
                    let timestamp_a = self
                        .notifications
                        .get(a)
                        .and_then(|n| n.effective_timestamp())
                        .unwrap_or(DateTime::<Utc>::MIN_UTC);
                    let timestamp_b = self
                        .notifications
                        .get(b)
                        .and_then(|n| n.effective_timestamp())
                        .unwrap_or(DateTime::<Utc>::MIN_UTC);
                    timestamp_b.cmp(&timestamp_a) // Descending order (newest first)
                });

                // Find the latest timestamp for this repository
                let latest_timestamp = notif_indices
                    .iter()
                    .filter_map(|&idx| {
                        self.notifications
                            .get(idx)
                            .and_then(|n| n.effective_timestamp())
                    })
                    .max()
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);

                (repo_name, notif_indices, latest_timestamp)
            })
            .collect();

        // Sort repositories by their latest notification timestamp (newest first)
        repo_list.sort_by(|a, b| b.2.cmp(&a.2)); // Descending order (newest first)

        // Build tree items: repository headers and their notifications
        for (repo_name, notif_indices, _) in repo_list {
            // Add repository header
            self.tree_items
                .push(TreeItem::RepositoryHeader(repo_name.clone()));

            // Add notifications if repository is expanded (default: expanded)
            let is_expanded = *self.expanded_repos.get(&repo_name).unwrap_or(&true);
            if is_expanded {
                for &notif_idx in &notif_indices {
                    self.tree_items.push(TreeItem::Notification(notif_idx));
                }
            }
        }
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

        // If the selected item is a Notification, walk backwards to find the parent RepositoryHeader
        for i in (0..self.selected_index).rev() {
            if let Some(TreeItem::RepositoryHeader(repo)) = self.tree_items.get(i) {
                return Some(repo.as_str());
            }
        }

        None
    }

    pub fn move_up(&mut self) {
        if self.tree_items.is_empty() {
            return;
        }

        // Early return if already at the first item
        if self.selected_index == 0 {
            return;
        }

        // Find the previous notification, skipping folders
        for i in (0..self.selected_index).rev() {
            if matches!(self.tree_items.get(i), Some(TreeItem::Notification(_))) {
                self.selected_index = i;
                return;
            }
        }
        // If no previous notification found, stay at current position
        // But if currently on a folder, move to first notification
        if matches!(
            self.tree_items.get(self.selected_index),
            Some(TreeItem::RepositoryHeader(_))
        ) {
            if let Some(first_notif_idx) = self
                .tree_items
                .iter()
                .position(|item| matches!(item, TreeItem::Notification(_)))
            {
                self.selected_index = first_notif_idx;
            }
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

        // Find the next notification, skipping folders
        for i in (self.selected_index + 1)..self.tree_items.len() {
            if matches!(self.tree_items.get(i), Some(TreeItem::Notification(_))) {
                self.selected_index = i;
                return;
            }
        }
        // If no next notification found, stay at current position
        // But if currently on a folder, move to last notification
        if matches!(
            self.tree_items.get(self.selected_index),
            Some(TreeItem::RepositoryHeader(_))
        ) {
            if let Some(last_notif_idx) = self
                .tree_items
                .iter()
                .rposition(|item| matches!(item, TreeItem::Notification(_)))
            {
                self.selected_index = last_notif_idx;
            }
        }
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_notifications.len()
    }

    pub fn show_preview(&self) -> bool {
        self.preview_mode != PreviewMode::Off
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
}
