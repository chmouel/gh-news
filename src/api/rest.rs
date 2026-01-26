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

    pub fn new(config: &Config) -> Result<Self> {
        let token = get_github_token()?;
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

    pub fn get_issue(&self, owner: &str, repo: &str, number: u64) -> Result<Value> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.api_base, owner, repo, number
        );
        self.get_json(&url)
    }

    pub fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<Value> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.api_base, owner, repo, number
        );
        self.get_json(&url)
    }

    pub fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<Value> {
        let url = format!("{}/repos/{}/{}/commits/{}", self.api_base, owner, repo, sha);
        self.get_json(&url)
    }

    pub fn mark_notification_read(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        self.send_no_content(self.client.patch(&url))
    }

    pub fn mark_thread_done(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        self.send_no_content(self.client.delete(&url))
    }

    pub fn get_vulnerability_alert_by_url(&self, url: &str) -> Result<Value> {
        self.get_json(url)
    }

    pub fn get_json_by_url(&self, url: &str) -> Result<Value> {
        self.get_json(url)
    }
}
