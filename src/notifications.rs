use crate::api::GitHubClient;
use crate::config::Config;
use crate::error::Result;
use crate::models::Notification;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStage {
    FetchNotifications,
    ResolveActionRepos,
    FetchWorkflowRuns,
    FetchActivityEvents,
    ResolveWatchRepos,
    FetchWatchRepoEvents,
    UpdateLocalState,
}

impl RefreshStage {
    pub fn loading_message(self) -> &'static str {
        match self {
            Self::FetchNotifications => "Refreshing GitHub notifications...",
            Self::ResolveActionRepos => "Resolving workflow run repositories...",
            Self::FetchWorkflowRuns => "Refreshing workflow runs...",
            Self::FetchActivityEvents => "Refreshing activity events...",
            Self::ResolveWatchRepos => "Resolving watched repositories...",
            Self::FetchWatchRepoEvents => "Refreshing watched-repo events...",
            Self::UpdateLocalState => "Updating local state...",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NotificationFetchOptions {
    pub show_all: bool,
    pub participating: bool,
    pub max_notifications: Option<usize>,
    pub per_page: usize,
}

impl NotificationFetchOptions {
    pub fn effective_per_page(self) -> usize {
        let max_notifications = self.max_notifications.unwrap_or(usize::MAX);
        let per_page = self.per_page.max(1);
        per_page.min(max_notifications.max(1))
    }
}

pub fn fetch_notifications(
    client: &GitHubClient,
    options: NotificationFetchOptions,
) -> Result<Vec<Notification>> {
    NotificationFetcher::new(client, options).fetch()
}

/// Fetch standard notifications plus all opt-in extra sources in one call.
/// This is the single fetch pipeline shared by the initial load, the
/// cache-hit background refresh, and the auto-refresh timer.
pub fn fetch_all_notifications(
    client: &GitHubClient,
    config: &Config,
    options: NotificationFetchOptions,
) -> Result<Vec<Notification>> {
    let mut notifications = fetch_notifications(client, options)?;
    let extra = fetch_extra_sources(client, config, &notifications);
    notifications.extend(extra);
    Ok(notifications)
}

pub fn refresh_stages(config: &Config) -> Vec<RefreshStage> {
    let mut stages = vec![RefreshStage::FetchNotifications];

    if config.enable_actions {
        if has_glob_patterns(&config.actions_repos) {
            stages.push(RefreshStage::ResolveActionRepos);
        }
        stages.push(RefreshStage::FetchWorkflowRuns);
    }

    if config.enable_events {
        stages.push(RefreshStage::FetchActivityEvents);
    }

    if !config.watch_repos.is_empty() {
        if has_glob_patterns(&config.watch_repos) {
            stages.push(RefreshStage::ResolveWatchRepos);
        }
        stages.push(RefreshStage::FetchWatchRepoEvents);
    }

    stages.push(RefreshStage::UpdateLocalState);
    stages
}

/// Fetch additional notifications from opt-in sources (Actions, Events).
///
/// These are merged with standard notifications after the main fetch.
/// Errors from individual sources are silently ignored to avoid
/// disrupting the main notification flow.
pub fn fetch_extra_sources(
    client: &GitHubClient,
    config: &Config,
    standard_notifications: &[Notification],
) -> Vec<Notification> {
    fetch_extra_sources_with_progress(client, config, standard_notifications, |_| Ok(()))
        .unwrap_or_default()
}

pub fn fetch_extra_sources_with_progress<F>(
    client: &GitHubClient,
    config: &Config,
    standard_notifications: &[Notification],
    mut on_stage: F,
) -> Result<Vec<Notification>>
where
    F: FnMut(RefreshStage) -> Result<()>,
{
    let mut extra = Vec::new();

    if config.enable_actions {
        let repos: Vec<String> = if config.actions_repos.is_empty() {
            // Derive unique repos from existing notifications
            let mut repos: Vec<String> = standard_notifications
                .iter()
                .map(|n| n.repo_full_name().to_string())
                .collect();
            repos.sort();
            repos.dedup();
            repos
        } else {
            if has_glob_patterns(&config.actions_repos) {
                on_stage(RefreshStage::ResolveActionRepos)?;
            }
            // Expand glob patterns via the GitHub API
            expand_repo_globs(client, &config.actions_repos)
        };

        on_stage(RefreshStage::FetchWorkflowRuns)?;
        if let Ok(runs) = crate::workflow_runs::fetch_workflow_run_notifications(
            client,
            &repos,
            config.actions_failed_only,
        ) {
            extra.extend(runs);
        }
    }

    if config.enable_events {
        on_stage(RefreshStage::FetchActivityEvents)?;
        if let Ok(events) = crate::events::fetch_activity_events(client, &config.event_types) {
            extra.extend(events);
        }
    }

    if !config.watch_repos.is_empty() {
        if has_glob_patterns(&config.watch_repos) {
            on_stage(RefreshStage::ResolveWatchRepos)?;
        }
        let repos = expand_repo_globs(client, &config.watch_repos);
        on_stage(RefreshStage::FetchWatchRepoEvents)?;
        if let Ok(events) =
            crate::events::fetch_watch_repo_events(client, &repos, &config.event_types)
        {
            extra.extend(events);
        }
    }

    // Deduplicate events that appear in both received_events and repo events.
    let mut seen_event_ids = std::collections::HashSet::new();
    extra.retain(|n| {
        let raw_id =
            n.id.strip_prefix("event-")
                .or_else(|| n.id.strip_prefix("repo-event-"))
                .unwrap_or(&n.id);
        seen_event_ids.insert(raw_id.to_string())
    });

    Ok(extra)
}

/// Expand glob patterns in actions_repos into concrete repo names.
///
/// Exact repo names (no wildcards) are passed through as-is.
/// Patterns with `*` or `?` trigger an API call to list repos for the
/// owner/org prefix, then match the results against the pattern.
fn expand_repo_globs(client: &GitHubClient, patterns: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for pattern in patterns {
        if pattern.contains('*') || pattern.contains('?') {
            // Extract the owner prefix (before the first /)
            let owner = match pattern.split('/').next() {
                Some(o) if !o.contains('*') && !o.contains('?') => o,
                _ => continue, // Skip patterns without a concrete owner
            };

            // Fetch repos for this owner via the GitHub API
            let repos = match client.list_owner_repos(owner) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Build a regex from the glob
            let regex_str = format!(
                "(?i)^{}$",
                regex::escape(pattern)
                    .replace(r"\*", ".*")
                    .replace(r"\?", ".")
            );
            if let Ok(re) = regex::Regex::new(&regex_str) {
                for repo in &repos {
                    if re.is_match(repo) && !result.contains(repo) {
                        result.push(repo.clone());
                    }
                }
            }
        } else if !result.contains(pattern) {
            result.push(pattern.clone());
        }
    }
    result
}

fn has_glob_patterns(patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern.contains('*') || pattern.contains('?'))
}

struct NotificationFetcher<'a> {
    client: &'a GitHubClient,
    options: NotificationFetchOptions,
}

impl<'a> NotificationFetcher<'a> {
    fn new(client: &'a GitHubClient, options: NotificationFetchOptions) -> Self {
        Self { client, options }
    }

    fn fetch(&self) -> Result<Vec<Notification>> {
        let max_notifications = self.options.max_notifications.unwrap_or(usize::MAX);
        if max_notifications == 0 {
            return Ok(Vec::new());
        }

        let per_page = self.options.effective_per_page();
        let mut all_notifications = Vec::new();

        for page in 1.. {
            let notifications = self.client.get_notifications(
                self.options.show_all,
                self.options.participating,
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

            all_notifications.extend(notifications.into_iter().take(remaining));
        }

        Ok(all_notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    struct TestServer {
        base_url: String,
        handle: thread::JoinHandle<()>,
    }

    impl TestServer {
        fn join(self) {
            self.handle.join().unwrap();
        }
    }

    fn spawn_json_server(routes: Vec<(String, u16, String)>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let base_url = format!("http://{}", address);
        let route_map: HashMap<String, (u16, String)> =
            routes
                .into_iter()
                .fold(HashMap::new(), |mut map, (path, status, body)| {
                    map.insert(path, (status, body));
                    map
                });

        let handle = thread::spawn(move || {
            for _ in 0..route_map.len() {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let path = request_line.split_whitespace().nth(1).unwrap();

                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }

                let (status, body) = route_map
                    .get(path)
                    .cloned()
                    .unwrap_or_else(|| (404, "[]".to_string()));
                let status_text = match status {
                    200 => "OK",
                    404 => "Not Found",
                    _ => "Error",
                };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    status_text,
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        TestServer { base_url, handle }
    }

    #[test]
    fn expand_repo_globs_matches_repos_from_later_pages() {
        let first_page = (0..100)
            .map(|i| serde_json::json!({ "full_name": format!("owner/repo-{i:03}") }))
            .collect::<Vec<_>>();
        let second_page = vec![serde_json::json!({ "full_name": "owner/deploy-prod" })];
        let server = spawn_json_server(vec![
            (
                "/orgs/owner/repos?per_page=100&type=all&page=1".to_string(),
                200,
                serde_json::to_string(&first_page).unwrap(),
            ),
            (
                "/orgs/owner/repos?per_page=100&type=all&page=2".to_string(),
                200,
                serde_json::to_string(&second_page).unwrap(),
            ),
        ]);
        let client = GitHubClient::new_test_with_base(server.base_url.clone());

        let repos = expand_repo_globs(&client, &[String::from("owner/deploy-*")]);

        assert_eq!(repos, vec![String::from("owner/deploy-prod")]);

        server.join();
    }

    #[test]
    fn refresh_stages_include_only_base_steps_by_default() {
        assert_eq!(
            refresh_stages(&Config::default()),
            vec![
                RefreshStage::FetchNotifications,
                RefreshStage::UpdateLocalState,
            ]
        );
    }

    #[test]
    fn refresh_stages_include_verbose_optional_steps() {
        let config = Config {
            enable_actions: true,
            actions_repos: vec!["owner/*".to_string()],
            enable_events: true,
            watch_repos: vec!["owner/repo".to_string(), "org/*".to_string()],
            ..Config::default()
        };

        assert_eq!(
            refresh_stages(&config),
            vec![
                RefreshStage::FetchNotifications,
                RefreshStage::ResolveActionRepos,
                RefreshStage::FetchWorkflowRuns,
                RefreshStage::FetchActivityEvents,
                RefreshStage::ResolveWatchRepos,
                RefreshStage::FetchWatchRepoEvents,
                RefreshStage::UpdateLocalState,
            ]
        );
    }
}
