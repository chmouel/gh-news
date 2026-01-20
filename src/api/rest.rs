use crate::api::get_github_token;
use crate::config::Config;
use crate::error::{ApiError, Error, Result};
use crate::models::Notification;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde_json::Value;
use std::time::Duration;

const API_VERSION: &str = "2022-11-28";

#[derive(Clone)]
pub struct GitHubClient {
    client: Client,
    api_base: String,
}

impl GitHubClient {
    pub fn new(config: &Config) -> Result<Self> {
        let token = get_github_token()?;
        let mut headers = HeaderMap::new();

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|_| {
                ApiError::HttpStatus {
                    status: 0,
                    message: "Invalid token format".to_string(),
                }
            })?,
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_str(API_VERSION).map_err(|e| ApiError::HttpStatus {
                status: 0,
                message: format!("Invalid API version header: {}", e),
            })?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str("gh-news/0.1.0").map_err(|e| ApiError::HttpStatus {
                status: 0,
                message: format!("Invalid user agent: {}", e),
            })?,
        );

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

        let mut request = self.client.get(&url).query(&[
            ("all", all.to_string()),
            ("participating", participating.to_string()),
        ]);

        if let Some(per_page) = per_page {
            request = request.query(&[("per_page", per_page.to_string())]);
        } else {
            request = request.query(&[("per_page", "50")]);
        }

        if let Some(page) = page {
            request = request.query(&[("page", page.to_string())]);
        }

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

        let response = self
            .client
            .put(&url)
            .json(&payload)
            .send()
            .map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        Ok(())
    }

    pub fn get_issue(&self, owner: &str, repo: &str, number: u64) -> Result<Value> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.api_base, owner, repo, number
        );
        let response = self.client.get(&url).send().map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        response.json().map_err(Error::from)
    }

    pub fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<Value> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.api_base, owner, repo, number
        );
        let response = self.client.get(&url).send().map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        response.json().map_err(Error::from)
    }

    pub fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<Value> {
        let url = format!("{}/repos/{}/{}/commits/{}", self.api_base, owner, repo, sha);
        let response = self.client.get(&url).send().map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        response.json().map_err(Error::from)
    }

    pub fn mark_notification_read(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        let response = self.client.patch(&url).send().map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        Ok(())
    }

    pub fn mark_thread_done(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/notifications/threads/{}", self.api_base, thread_id);
        let response = self.client.delete(&url).send().map_err(Error::from)?;

        // DELETE returns 204 No Content on success
        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        Ok(())
    }

    pub fn get_vulnerability_alert_by_url(&self, url: &str) -> Result<Value> {
        let response = self.client.get(url).send().map_err(Error::from)?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ApiError::HttpStatus {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        response.json().map_err(Error::from)
    }
}
