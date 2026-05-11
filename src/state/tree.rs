use crate::models::Notification;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

use super::{OrgGroupingMode, OrgHeaderInfo, RepoHeaderInfo, TreeItem};

pub struct TreeBuilder<'a> {
    notifications: &'a [Notification],
    filtered_notifications: &'a [usize],
    expanded_repos: &'a HashMap<String, bool>,
    expanded_orgs: &'a HashMap<String, bool>,
    pinned_ids: &'a HashSet<String>,
    org_grouping: OrgGroupingMode,
}

impl<'a> TreeBuilder<'a> {
    pub fn new(
        notifications: &'a [Notification],
        filtered_notifications: &'a [usize],
        expanded_repos: &'a HashMap<String, bool>,
        expanded_orgs: &'a HashMap<String, bool>,
        pinned_ids: &'a HashSet<String>,
        org_grouping: OrgGroupingMode,
    ) -> Self {
        Self {
            notifications,
            filtered_notifications,
            expanded_repos,
            expanded_orgs,
            pinned_ids,
            org_grouping,
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

        let mut org_counts: HashMap<String, usize> = HashMap::new();
        for idx in &regular_indices {
            if let Some(notif) = self.notifications.get(*idx) {
                if notif.repository.owner.owner_type == "Organization" {
                    *org_counts
                        .entry(notif.repository.owner.login.clone())
                        .or_insert(0) += 1;
                }
            }
        }

        let use_org_grouping = !org_counts.is_empty()
            && match self.org_grouping {
                OrgGroupingMode::Off => false,
                OrgGroupingMode::Always => true,
                OrgGroupingMode::Auto => org_counts.values().any(|&c| c > 1),
            };

        if !use_org_grouping {
            let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();
            for idx in regular_indices {
                if let Some(notif) = self.notifications.get(idx) {
                    let repo_name = notif.repo_full_name().to_string();
                    repo_groups.entry(repo_name).or_default().push(idx);
                }
            }

            self.append_repos(&mut tree_items, &self.sort_repo_groups(repo_groups), None);
            return tree_items;
        }

        let mut org_groups: HashMap<String, Vec<usize>> = HashMap::new();
        let mut non_org_indices: Vec<usize> = Vec::new();

        for idx in regular_indices {
            if let Some(notif) = self.notifications.get(idx) {
                if notif.repository.owner.owner_type == "Organization" {
                    org_groups
                        .entry(notif.repository.owner.login.clone())
                        .or_default()
                        .push(idx);
                } else {
                    non_org_indices.push(idx);
                }
            }
        }

        let mut org_list: Vec<(String, Vec<usize>, DateTime<Utc>)> = org_groups
            .into_iter()
            .map(|(org, indices)| {
                let latest_timestamp = indices
                    .iter()
                    .filter_map(|&idx| {
                        self.notifications
                            .get(idx)
                            .and_then(|n| n.effective_timestamp())
                    })
                    .max()
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);
                (org, indices, latest_timestamp)
            })
            .collect();

        org_list.sort_by_key(|o| std::cmp::Reverse(o.2));

        for (org, indices, _) in org_list {
            let notification_count = indices.len();

            tree_items.push(TreeItem::OrgHeader(OrgHeaderInfo {
                login: org.clone(),
                notification_count,
            }));

            let is_org_expanded = *self.expanded_orgs.get(&org).unwrap_or(&true);
            if is_org_expanded {
                let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();
                for idx in indices {
                    if let Some(notif) = self.notifications.get(idx) {
                        let repo_name = notif.repo_full_name().to_string();
                        repo_groups.entry(repo_name).or_default().push(idx);
                    }
                }

                self.append_repos(
                    &mut tree_items,
                    &self.sort_repo_groups(repo_groups),
                    Some(&org),
                );
            }
        }

        if !non_org_indices.is_empty() {
            let mut repo_groups: HashMap<String, Vec<usize>> = HashMap::new();
            for idx in non_org_indices {
                if let Some(notif) = self.notifications.get(idx) {
                    let repo_name = notif.repo_full_name().to_string();
                    repo_groups.entry(repo_name).or_default().push(idx);
                }
            }

            self.append_repos(&mut tree_items, &self.sort_repo_groups(repo_groups), None);
        }

        tree_items
    }

    fn sort_repo_groups(
        &self,
        repo_groups: HashMap<String, Vec<usize>>,
    ) -> Vec<(String, Vec<usize>, DateTime<Utc>)> {
        let mut repo_list: Vec<(String, Vec<usize>, DateTime<Utc>)> = repo_groups
            .into_iter()
            .map(|(repo_name, mut notif_indices)| {
                notif_indices.sort_by_key(|&idx| {
                    std::cmp::Reverse(
                        self.notifications
                            .get(idx)
                            .and_then(|n| n.effective_timestamp())
                            .unwrap_or(DateTime::<Utc>::MIN_UTC),
                    )
                });

                let latest_timestamp = notif_indices
                    .first()
                    .and_then(|&idx| self.notifications.get(idx))
                    .and_then(|n| n.effective_timestamp())
                    .unwrap_or(DateTime::<Utc>::MIN_UTC);

                (repo_name, notif_indices, latest_timestamp)
            })
            .collect();

        repo_list.sort_by_key(|r| std::cmp::Reverse(r.2));
        repo_list
    }

    fn append_repos(
        &self,
        tree_items: &mut Vec<TreeItem>,
        repo_list: &[(String, Vec<usize>, DateTime<Utc>)],
        current_org: Option<&str>,
    ) {
        for (repo_name, notif_indices, _) in repo_list {
            // Strip org prefix from display name when under an org header
            let display_name = current_org
                .and_then(|org| {
                    repo_name
                        .split_once('/')
                        .and_then(|(owner, repo)| (owner == org).then_some(repo))
                })
                .unwrap_or(repo_name)
                .to_string();

            let notification_count = notif_indices.len();

            tree_items.push(TreeItem::RepositoryHeader(RepoHeaderInfo {
                full_name: repo_name.clone(),
                display_name,
                notification_count,
            }));

            let is_expanded = *self.expanded_repos.get(repo_name).unwrap_or(&true);
            if is_expanded {
                for &notif_idx in notif_indices {
                    tree_items.push(TreeItem::Notification(notif_idx));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NotificationType, Owner, Repository, Subject};
    use chrono::TimeZone;

    fn notif(id: &str, owner: &str, repo: &str, owner_type: &str, ts: i64) -> Notification {
        Notification {
            id: id.to_string(),
            unread: true,
            last_read_at: None,
            updated_at: Some(Utc.timestamp_opt(ts, 0).unwrap()),
            reason: "subscribed".to_string(),
            repository: Repository {
                id: 1,
                name: repo.to_string(),
                full_name: format!("{}/{}", owner, repo),
                owner: Owner {
                    login: owner.to_string(),
                    id: 1,
                    owner_type: owner_type.to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: "t".to_string(),
                subject_type: NotificationType::Issue,
                url: None,
                latest_comment_url: None,
            },
            latest_comment_url: None,
            author: None,
            context: None,
            event_body: None,
        }
    }

    fn build(notifications: Vec<Notification>, mode: OrgGroupingMode) -> Vec<TreeItem> {
        let filtered: Vec<usize> = (0..notifications.len()).collect();
        TreeBuilder::new(
            &notifications,
            &filtered,
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
            mode,
        )
        .build()
    }

    #[test]
    fn org_grouping_auto_disabled_when_no_org_has_multiple() {
        let items = build(
            vec![
                notif("1", "org1", "r1", "Organization", 10),
                notif("2", "org2", "r2", "Organization", 20),
            ],
            OrgGroupingMode::Auto,
        );
        assert!(!items.iter().any(|i| matches!(i, TreeItem::OrgHeader(_))));
    }

    #[test]
    fn org_grouping_auto_enabled_when_org_has_multiple() {
        let items = build(
            vec![
                notif("1", "org1", "r1", "Organization", 10),
                notif("2", "org1", "r2", "Organization", 20),
            ],
            OrgGroupingMode::Auto,
        );
        assert!(items
            .iter()
            .any(|i| matches!(i, TreeItem::OrgHeader(info) if info.login == "org1")));
    }

    #[test]
    fn org_grouping_off_never_emits_org_header() {
        let items = build(
            vec![
                notif("1", "org1", "r1", "Organization", 10),
                notif("2", "org1", "r2", "Organization", 20),
            ],
            OrgGroupingMode::Off,
        );
        assert!(!items.iter().any(|i| matches!(i, TreeItem::OrgHeader(_))));
    }

    #[test]
    fn org_grouping_always_emits_org_header_when_applicable() {
        let items = build(
            vec![notif("1", "org1", "r1", "Organization", 10)],
            OrgGroupingMode::Always,
        );
        assert!(items
            .iter()
            .any(|i| matches!(i, TreeItem::OrgHeader(info) if info.login == "org1")));
    }
}
