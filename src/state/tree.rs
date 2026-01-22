use crate::models::Notification;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

use super::TreeItem;

pub struct TreeBuilder<'a> {
    notifications: &'a [Notification],
    filtered_notifications: &'a [usize],
    expanded_repos: &'a HashMap<String, bool>,
    pinned_ids: &'a HashSet<String>,
}

impl<'a> TreeBuilder<'a> {
    pub fn new(
        notifications: &'a [Notification],
        filtered_notifications: &'a [usize],
        expanded_repos: &'a HashMap<String, bool>,
        pinned_ids: &'a HashSet<String>,
    ) -> Self {
        Self {
            notifications,
            filtered_notifications,
            expanded_repos,
            pinned_ids,
        }
    }

    pub fn build(self) -> Vec<TreeItem> {
        let mut tree_items = Vec::new();

        let (pinned_indices, regular_indices): (Vec<usize>, Vec<usize>) =
            self.filtered_notifications.iter().partition(|&&idx| {
                self.notifications
                    .get(idx)
                    .map(|n| self.pinned_ids.contains(&n.id))
                    .unwrap_or(false)
            });

        if !pinned_indices.is_empty() {
            tree_items.push(TreeItem::PinnedHeader);

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
                tree_items.push(TreeItem::Notification(idx));
            }
        }

        let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for idx in regular_indices {
            if let Some(notif) = self.notifications.get(idx) {
                let repo_name = notif.repo_full_name().to_string();
                repo_groups.entry(repo_name).or_default().push(idx);
            }
        }

        let mut repo_list: Vec<(String, Vec<usize>, DateTime<Utc>)> = repo_groups
            .into_iter()
            .map(|(repo_name, mut notif_indices)| {
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
                    timestamp_b.cmp(&timestamp_a)
                });

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

        repo_list.sort_by(|a, b| b.2.cmp(&a.2));

        for (repo_name, notif_indices, _) in repo_list {
            tree_items.push(TreeItem::RepositoryHeader(repo_name.clone()));

            let is_expanded = *self.expanded_repos.get(&repo_name).unwrap_or(&true);
            if is_expanded {
                for &notif_idx in &notif_indices {
                    tree_items.push(TreeItem::Notification(notif_idx));
                }
            }
        }

        tree_items
    }
}
