use crate::api::get_github_token;
use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType};
use reqwest::blocking::Client as ReqwestClient;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use std::fmt;

#[derive(Debug, Clone)]
pub enum PreviewData {
    PullRequest {
        number: String,
        title: String,
        state: String,
        author: String,
        comments: u64,
        mergeable: String,
        body: String,
    },
    Issue {
        number: String,
        title: String,
        state: String,
        author: String,
        comments: u64,
        body: String,
    },
    Commit {
        sha: String,
        author: String,
        message: String,
        body: String,
    },
    Release {
        tag: String,
        name: String,
        published_at: String,
        prerelease: bool,
        body: String,
    },
    SecurityAlert {
        severity: String,
        vulnerability_count: u64,
        affected_packages: Vec<String>,
        body: String,
    },
    Generic {
        title: String,
        notification_type: NotificationType,
        body: String,
    },
}

impl fmt::Display for PreviewData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreviewData::PullRequest {
                number,
                title,
                state,
                author,
                comments,
                mergeable,
                body,
            } => {
                write!(
                    f,
                    "Pull Request #{} - {}\n\
                    State: {} | Author: {} | Comments: {} | Mergeable: {}\n\
                    {}\n\n\
                    {}\n",
                    number,
                    title,
                    state,
                    author,
                    comments,
                    mergeable,
                    "=".repeat(80),
                    body
                )
            }
            PreviewData::Issue {
                number,
                title,
                state,
                author,
                comments,
                body,
            } => {
                write!(
                    f,
                    "Issue #{} - {}\n\
                    State: {} | Author: {} | Comments: {}\n\
                    {}\n\n\
                    {}\n",
                    number,
                    title,
                    state,
                    author,
                    comments,
                    "=".repeat(80),
                    body
                )
            }
            PreviewData::Commit {
                sha,
                author,
                message,
                body: _,
            } => {
                write!(
                    f,
                    "Commit {}\n\
                    Author: {}\n\
                    {}\n\n\
                    {}\n",
                    &sha[..12.min(sha.len())],
                    author,
                    "=".repeat(80),
                    message
                )
            }
            PreviewData::Release {
                tag,
                name,
                published_at,
                prerelease,
                body,
            } => {
                write!(
                    f,
                    "Release {} - {}\n\
                    Tag: {} | Published: {} | Pre-release: {}\n\
                    {}\n\n\
                    {}\n",
                    name,
                    tag,
                    tag,
                    published_at,
                    if *prerelease { "Yes" } else { "No" },
                    "=".repeat(80),
                    body
                )
            }
            PreviewData::SecurityAlert {
                severity,
                vulnerability_count,
                affected_packages,
                body,
            } => {
                let packages_list = if affected_packages.is_empty() {
                    "None specified".to_string()
                } else {
                    affected_packages.join(", ")
                };
                write!(
                    f,
                    "Security Alert\n\
                    Severity: {} | Vulnerabilities: {} | Affected Packages: {}\n\
                    {}\n\n\
                    {}\n",
                    severity,
                    vulnerability_count,
                    packages_list,
                    "=".repeat(80),
                    body
                )
            }
            PreviewData::Generic {
                title,
                notification_type,
                body,
            } => {
                write!(
                    f,
                    "Preview not supported for {} notifications\n\nTitle: {}\n\n{}",
                    notification_type, title, body
                )
            }
        }
    }
}

pub struct PreviewFetcher;

impl PreviewFetcher {
    pub fn fetch_preview(
        client: &GitHubClient,
        notification: &Notification,
    ) -> Result<PreviewData> {
        let repo_full_name = notification.repo_full_name();
        let notification_type = notification.notification_type();

        let preview_data = match notification_type {
            NotificationType::Issue => {
                // Extract number from subject URL
                let number =
                    Self::extract_number_from_url(notification.subject_url(), notification_type)?;
                Self::fetch_issue_preview(client, repo_full_name, number)?
            }
            NotificationType::PullRequest => {
                // Extract number from subject URL
                let number =
                    Self::extract_number_from_url(notification.subject_url(), notification_type)?;
                Self::fetch_pr_preview(client, repo_full_name, number)?
            }
            NotificationType::Commit => {
                // Extract SHA from subject URL
                let number =
                    Self::extract_number_from_url(notification.subject_url(), notification_type)?;
                Self::fetch_commit_preview(client, repo_full_name, &number)?
            }
            NotificationType::Release => {
                // For releases, we need to fetch by release ID first, then get tag
                // Releases don't have a "number" - they have a release ID in the URL
                if let Some(url) = notification.subject_url() {
                    // Try to get release by fetching the URL directly
                    Self::fetch_release_by_url(client, url)?
                } else {
                    PreviewData::Generic {
                        title: notification.title().to_string(),
                        notification_type,
                        body: format!(
                            "Release preview unavailable - no URL\n\nRepository: {}",
                            repo_full_name
                        ),
                    }
                }
            }
            NotificationType::Discussion => PreviewData::Generic {
                title: notification.title().to_string(),
                notification_type,
                body: format!(
                    "Discussion preview not yet implemented\n\nRepository: {}",
                    repo_full_name
                ),
            },
            NotificationType::RepositoryVulnerabilityAlert => {
                if let Some(url) = notification.subject_url() {
                    Self::fetch_security_alert_preview(client, url)?
                } else {
                    PreviewData::Generic {
                        title: notification.title().to_string(),
                        notification_type,
                        body: format!(
                            "Security alert preview unavailable - no URL\n\nRepository: {}",
                            repo_full_name
                        ),
                    }
                }
            }
            _ => PreviewData::Generic {
                title: notification.title().to_string(),
                notification_type,
                body: format!("Repository: {}", repo_full_name),
            },
        };

        Ok(preview_data)
    }

    fn extract_number_from_url(
        url: Option<&str>,
        notification_type: NotificationType,
    ) -> Result<String> {
        let url =
            url.ok_or_else(|| crate::error::Error::Config("No subject URL available".to_string()))?;

        match notification_type {
            NotificationType::Commit => {
                // For commits, extract SHA from URL (last part, first 12 chars)
                // URL format: https://api.github.com/repos/owner/repo/commits/abc123...
                url.split('/')
                    .next_back()
                    .map(|s| s.chars().take(12).collect())
                    .ok_or_else(|| crate::error::Error::Config("Invalid commit URL".to_string()))
            }
            NotificationType::Release => {
                // For releases, we need to fetch the release to get the tag
                // But for now, try to extract from URL or return error
                // URL format: https://api.github.com/repos/owner/repo/releases/{id}
                // We'll need to fetch the release to get the tag_name
                Err(crate::error::Error::Config(
                    "Release tag extraction requires API call".to_string(),
                ))
            }
            _ => {
                // For issues/PRs, extract number from URL
                // URL format: https://api.github.com/repos/owner/repo/issues/123
                // or: https://api.github.com/repos/owner/repo/pulls/456
                url.split('/')
                    .next_back()
                    .map(|s| s.to_string())
                    .ok_or_else(|| crate::error::Error::Config("Invalid URL format".to_string()))
            }
        }
    }

    fn fetch_issue_preview(
        client: &GitHubClient,
        repo: &str,
        number: String,
    ) -> Result<PreviewData> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            return Err(crate::error::Error::Config(
                "Invalid repo format".to_string(),
            ));
        }

        let issue_num: u64 = number
            .parse()
            .map_err(|_| crate::error::Error::Config("Invalid issue number".to_string()))?;

        let issue = client.get_issue(parts[0], parts[1], issue_num)?;

        let title = issue
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("No title")
            .to_string();
        let body = issue
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("No description")
            .to_string();
        let state = issue
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let author = issue
            .get("user")
            .and_then(|v| v.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let comments = issue.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);

        Ok(PreviewData::Issue {
            number,
            title,
            state,
            author,
            comments,
            body,
        })
    }

    fn fetch_pr_preview(client: &GitHubClient, repo: &str, number: String) -> Result<PreviewData> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            return Err(crate::error::Error::Config(
                "Invalid repo format".to_string(),
            ));
        }

        let pr_num: u64 = number
            .parse()
            .map_err(|_| crate::error::Error::Config("Invalid PR number".to_string()))?;

        let pr = client.get_pr(parts[0], parts[1], pr_num)?;

        let title = pr
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("No title")
            .to_string();
        let body = pr
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("No description")
            .to_string();
        let state = pr
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let author = pr
            .get("user")
            .and_then(|v| v.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let comments = pr.get("comments").and_then(|v| v.as_u64()).unwrap_or(0);
        let mergeable = pr
            .get("mergeable")
            .and_then(|v| v.as_bool())
            .map(|b| if b { "Yes" } else { "No" })
            .unwrap_or("Unknown")
            .to_string();

        Ok(PreviewData::PullRequest {
            number,
            title,
            state,
            author,
            comments,
            mergeable,
            body,
        })
    }

    fn fetch_commit_preview(client: &GitHubClient, repo: &str, sha: &str) -> Result<PreviewData> {
        let parts: Vec<&str> = repo.split('/').collect();
        if parts.len() != 2 {
            return Err(crate::error::Error::Config(
                "Invalid repo format".to_string(),
            ));
        }

        let commit = client.get_commit(parts[0], parts[1], sha)?;

        let message = commit
            .get("commit")
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("No message")
            .to_string();
        let author = commit
            .get("commit")
            .and_then(|v| v.get("author"))
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let message_clone = message.clone();
        Ok(PreviewData::Commit {
            sha: sha.to_string(),
            author,
            message: message_clone.clone(),
            body: message_clone,
        })
    }

    fn fetch_release_by_url(_client: &GitHubClient, url: &str) -> Result<PreviewData> {
        // Fetch release by API URL
        // URL format: https://api.github.com/repos/owner/repo/releases/{id}
        let token = get_github_token()?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|_| crate::error::Error::Config("Invalid token format".to_string()))?,
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_str("2022-11-28")
                .map_err(|_| crate::error::Error::Config("Invalid API version".to_string()))?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str("gh-news/0.1.0")
                .map_err(|_| crate::error::Error::Config("Invalid user agent".to_string()))?,
        );

        let http_client = ReqwestClient::builder()
            .default_headers(headers)
            .build()
            .map_err(crate::error::Error::Http)?;

        let response = http_client
            .get(url)
            .send()
            .map_err(crate::error::Error::Http)?;

        if !response.status().is_success() {
            return Err(crate::error::Error::Config(format!(
                "Failed to fetch release: {}",
                response.status()
            )));
        }

        let release: serde_json::Value = response.json().map_err(crate::error::Error::Http)?;

        let tag = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let name = release
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&tag)
            .to_string();
        let body = release
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("No release notes")
            .to_string();
        let prerelease = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        Ok(PreviewData::Release {
            tag,
            name,
            published_at,
            prerelease,
            body,
        })
    }

    fn fetch_security_alert_preview(client: &GitHubClient, url: &str) -> Result<PreviewData> {
        // Fetch the security alert from the URL
        // Note: GitHub's vulnerability alert notifications may point to Dependabot alerts
        // The URL structure might be: /repos/{owner}/{repo}/vulnerability-alerts/{number}
        // or it might point to a Dependabot alert directly
        let alert_data = match client.get_vulnerability_alert_by_url(url) {
            Ok(data) => data,
            Err(e) => {
                // If fetching fails, return a generic security alert with the notification title
                // This handles cases where the API endpoint might be different or require special permissions
                return Ok(PreviewData::SecurityAlert {
                    severity: "Unknown".to_string(),
                    vulnerability_count: 1,
                    affected_packages: Vec::new(),
                    body: format!(
                        "Security vulnerability detected.\n\nUnable to fetch detailed information: {}\n\nURL: {}",
                        e, url
                    ),
                });
            }
        };

        // Parse the alert data
        // Note: GitHub's vulnerability alert API structure may vary
        // We'll extract what we can from the response
        let severity = alert_data
            .get("severity")
            .and_then(|v| v.as_str())
            .or_else(|| {
                alert_data
                    .get("security_vulnerability")
                    .and_then(|v| v.get("severity"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("Unknown")
            .to_string();

        // Try to get vulnerability count - this might be in different places
        let vulnerability_count = alert_data
            .get("vulnerability_count")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                // Try alternative paths
                alert_data
                    .get("vulnerabilities")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len() as u64)
            })
            .or_else(|| {
                // Check if it's a single vulnerability alert
                if alert_data.get("security_vulnerability").is_some() {
                    Some(1)
                } else {
                    None
                }
            })
            .unwrap_or(1);

        // Extract affected packages
        let mut affected_packages = Vec::new();
        if let Some(packages) = alert_data.get("vulnerable_packages") {
            if let Some(arr) = packages.as_array() {
                for pkg in arr {
                    if let Some(name) = pkg.get("name").and_then(|v| v.as_str()) {
                        affected_packages.push(name.to_string());
                    }
                }
            }
        } else if let Some(package) = alert_data.get("package") {
            if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
                affected_packages.push(name.to_string());
            } else if let Some(ecosystem) = package.get("ecosystem").and_then(|v| v.as_str()) {
                // Try to get package name from ecosystem
                if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
                    affected_packages.push(format!("{}:{}", ecosystem, name));
                }
            }
        } else if let Some(sec_vuln) = alert_data.get("security_vulnerability") {
            // Dependabot alert structure
            if let Some(package) = sec_vuln.get("package") {
                if let Some(name) = package.get("name").and_then(|v| v.as_str()) {
                    affected_packages.push(name.to_string());
                }
            }
        }

        // Get the alert description/body
        let body = alert_data
            .get("description")
            .and_then(|v| v.as_str())
            .or_else(|| alert_data.get("summary").and_then(|v| v.as_str()))
            .or_else(|| alert_data.get("body").and_then(|v| v.as_str()))
            .or_else(|| {
                // Try to get from security_vulnerability
                alert_data
                    .get("security_vulnerability")
                    .and_then(|v| v.get("advisory").and_then(|a| a.get("summary")))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or(
                "Security vulnerability detected. Please review the alert details on GitHub.",
            )
            .to_string();

        Ok(PreviewData::SecurityAlert {
            severity,
            vulnerability_count,
            affected_packages,
            body,
        })
    }
}
