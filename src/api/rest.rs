use crate::api::get_github_token;
use crate::config::Config;
use crate::error::{ApiError, Error, Result};
use crate::models::Notification;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

const API_VERSION: &str = "2022-11-28";
const DEFAULT_PER_PAGE: usize = 50;
const MAX_PER_PAGE: usize = 100;
const USER_AGENT_VALUE: &str = "gh-news/0.1.0";

struct NotificationQuery {
    all: bool,
    participating: bool,
    per_page: Option<usize>,
    page: Option<usize>,
}

impl NotificationQuery {
    fn apply(self, request: RequestBuilder) -> RequestBuilder {
        let per_page = self.per_page.unwrap_or(DEFAULT_PER_PAGE);
        let mut request = request.query(&[
            ("all", self.all.to_string()),
            ("participating", self.participating.to_string()),
            ("per_page", per_page.to_string()),
        ]);

        if let Some(page) = self.page {
            request = request.query(&[("page", page.to_string())]);
        }

        request
    }
}

fn is_synthetic_thread(id: &str) -> bool {
    id.starts_with("actions-") || id.starts_with("event-") || id.starts_with("repo-event-")
}

fn bearer_header_value(token: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|_| {
        ApiError::HttpStatus {
            status: 0,
            message: "Invalid token format".to_string(),
        }
        .into()
    })
}

fn default_headers(token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, bearer_header_value(token)?);
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static(API_VERSION),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    Ok(headers)
}

#[derive(Clone)]
pub struct GitHubClient {
    client: Client,
    api_base: String,
}

impl GitHubClient {
    /// Get the API base URL.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = request.send().map_err(Error::from)?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }
        Ok(response)
    }

    fn send_json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        let response = self.send(request)?;
        response.json().map_err(Error::from)
    }

    fn send_no_content(&self, request: RequestBuilder) -> Result<()> {
        self.send(request).map(|_| ())
    }

    fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.send_json(self.client.get(url))
    }

    /// Build a minimal client suitable for unit tests that never make real HTTP requests.
    #[cfg(test)]
    pub fn new_test() -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            api_base: "http://localhost".to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_test_with_base(api_base: impl Into<String>) -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            api_base: api_base.into(),
        }
    }

    pub fn new(config: &Config) -> Result<Self> {
        let token = get_github_token(&config.github_host)?;
        let headers = default_headers(&token)?;

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(config.api_timeout))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(Error::from)?;

        Ok(Self {
            client,
            api_base: config.github_api_base(),
        })
    }

    pub fn get_notifications(
        &self,
        all: bool,
        participating: bool,
        per_page: Option<usize>,
        page: Option<usize>,
    ) -> Result<Vec<Notification>> {
        let url = format!("{}/notifications", self.api_base);

        let request = NotificationQuery {
            all,
            participating,
            per_page,
            page,
        }
        .apply(self.client.get(&url));
        let response = self.send(request)?;
        response
            .json::<Vec<Notification>>()
            .map_err(|_| Error::Api(ApiError::InvalidResponse))
    }

    /// Fetch the author login from a comment or issue/PR API URL.
    /// Returns `None` if the URL is inaccessible or has no user field.
    pub fn get_comment_author(&self, url: &str) -> Result<Option<String>> {
        let value: Value = self.get_json(url)?;
        Ok(value
            .get("user")
            .and_then(|u| u.get("login"))
            .and_then(|l| l.as_str())
            .map(|s| s.to_string()))
    }

    pub fn mark_all_read(&self, last_read_at: Option<&str>) -> Result<()> {
        let url = format!("{}/notifications", self.api_base);
        let payload = if let Some(last_read_at) = last_read_at {
            serde_json::json!({
                "last_read_at": last_read_at,
                "read": true
            })
        } else {
            serde_json::json!({ "read": true })
        };

        self.send_no_content(self.client.put(&url).json(&payload))
    }

    pub fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<Value> {
        let url = format!("{}/repos/{}/{}/commits/{}", self.api_base, owner, repo, sha);
        self.get_json(&url)
    }

    pub fn mark_notification_read(&self, thread_id: &str) -> Result<()> {
        // Synthetic notifications (actions-*, event-*, repo-event-*) have no real thread
        if is_synthetic_thread(thread_id) {
            return Ok(());
        }
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        self.send_no_content(self.client.patch(&url))
    }

    pub fn mark_thread_done(&self, thread_id: &str) -> Result<()> {
        // Synthetic notifications (actions-*, event-*, repo-event-*) have no real thread
        if is_synthetic_thread(thread_id) {
            return Ok(());
        }
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        self.send_no_content(self.client.delete(&url))
    }

    pub fn get_vulnerability_alert_by_url(&self, url: &str) -> Result<Value> {
        self.get_json(url)
    }

    pub fn get_json_by_url(&self, url: &str) -> Result<Value> {
        self.get_json(url)
    }

    /// Mute a repository by setting subscription to ignored.
    /// This prevents notifications for this repository until the user acts on it again.
    pub fn mute_repository(&self, owner: &str, repo: &str) -> Result<()> {
        let url = format!("{}/repos/{}/{}/subscription", self.api_base, owner, repo);
        let payload = serde_json::json!({
            "ignored": true
        });
        self.send_no_content(self.client.put(&url).json(&payload))
    }

    /// Mute a notification thread by setting subscription to ignored.
    /// This prevents future notifications for this thread until the user comments or is mentioned.
    pub fn mute_thread(&self, thread_id: &str) -> Result<()> {
        let url = format!(
            "{}/notifications/threads/{}/subscription",
            self.api_base, thread_id
        );
        let payload = serde_json::json!({
            "ignored": true
        });
        self.send_no_content(self.client.put(&url).json(&payload))
    }

    /// Fetch workflow runs from GitHub Actions.
    pub fn get_workflow_runs(
        &self,
        owner: &str,
        repo: &str,
        status: Option<&str>,
        per_page: usize,
    ) -> Result<Value> {
        let mut url = format!(
            "{}/repos/{}/{}/actions/runs?per_page={}",
            self.api_base, owner, repo, per_page
        );
        if let Some(s) = status {
            url.push_str(&format!("&status={}", s));
        }
        self.get_json(&url)
    }

    /// Fetch the authenticated user's login.
    pub fn get_authenticated_user(&self) -> Result<String> {
        let url = format!("{}/user", self.api_base);
        let user: Value = self.get_json(&url)?;
        user.get("login")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| crate::error::Error::Config("Could not get username".to_string()))
    }

    /// List repositories for an owner (user or organisation).
    ///
    /// Tries the org endpoint first; falls back to the user endpoint.
    /// Returns full_name values like "owner/repo".
    pub fn list_owner_repos(&self, owner: &str) -> Result<Vec<String>> {
        // Try org repos first, fall back to user repos
        let base_urls = [
            format!(
                "{}/orgs/{}/repos?per_page={}&type=all",
                self.api_base, owner, MAX_PER_PAGE
            ),
            format!(
                "{}/users/{}/repos?per_page={}&type=all",
                self.api_base, owner, MAX_PER_PAGE
            ),
        ];

        for base_url in &base_urls {
            let mut all_repos = Vec::new();

            for page in 1.. {
                let url = format!("{}&page={}", base_url, page);
                match self.get_json::<Value>(&url) {
                    Ok(repos) => {
                        let Some(arr) = repos.as_array() else {
                            return Err(Error::Api(ApiError::InvalidResponse));
                        };

                        if arr.is_empty() {
                            return Ok(all_repos);
                        }

                        for repo in arr {
                            if let Some(full_name) = repo.get("full_name").and_then(|v| v.as_str())
                            {
                                all_repos.push(full_name.to_string());
                            }
                        }

                        if arr.len() < MAX_PER_PAGE {
                            return Ok(all_repos);
                        }
                    }
                    Err(Error::Api(ApiError::HttpStatus { status: 404, .. })) if page == 1 => {
                        break;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        Ok(Vec::new())
    }

    /// Fetch events for a specific repository.
    pub fn get_repo_events(&self, owner: &str, repo: &str, per_page: usize) -> Result<Value> {
        let url = format!(
            "{}/repos/{}/{}/events?per_page={}",
            self.api_base, owner, repo, per_page
        );
        self.get_json(&url)
    }

    /// Fetch received events for a user.
    pub fn get_received_events(&self, username: &str, per_page: usize) -> Result<Value> {
        let url = format!(
            "{}/users/{}/received_events?per_page={}",
            self.api_base, username, per_page
        );
        self.get_json(&url)
    }

    /// Execute a GraphQL query against the GitHub API.
    ///
    /// GitHub Discussions are only available via GraphQL, so this method
    /// complements the REST helpers above. See
    /// https://github.com/chmouel/gh-news/pull/27 for discussion of the trade-off of keeping this in `rest.rs`.
    pub fn graphql(&self, query: &str, variables: serde_json::Value) -> Result<serde_json::Value> {
        let url = if self.api_base.contains("/api/v3") {
            // GHE: https://HOST/api/graphql
            self.api_base.replace("/api/v3", "/api/graphql")
        } else {
            // github.com: https://api.github.com/graphql
            format!("{}/graphql", self.api_base)
        };

        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let response: serde_json::Value = self.send_json(self.client.post(&url).json(&body))?;

        if let Some(errors) = response.get("errors") {
            let message = errors
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown GraphQL error");
            return Err(ApiError::GraphQL {
                message: message.to_string(),
            }
            .into());
        }

        response.get("data").cloned().ok_or_else(|| {
            ApiError::GraphQL {
                message: "Response missing 'data' field".to_string(),
            }
            .into()
        })
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
    fn list_owner_repos_paginates_until_short_page() {
        let first_page = (0..100)
            .map(|i| serde_json::json!({ "full_name": format!("owner/repo-{i:03}") }))
            .collect::<Vec<_>>();
        let second_page = vec![serde_json::json!({ "full_name": "owner/repo-100" })];
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

        let repos = client.list_owner_repos("owner").unwrap();

        assert_eq!(repos.len(), 101);
        assert_eq!(repos.first().unwrap(), "owner/repo-000");
        assert_eq!(repos.last().unwrap(), "owner/repo-100");

        server.join();
    }
}
