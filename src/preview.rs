use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType};
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
    Discussion {
        number: String,
        title: String,
        state: String,
        author: String,
        comments: u64,
        category: String,
        answered: bool,
        body: String,
        url: String,
    },
    Generic {
        title: String,
        body: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewHeaderKind {
    Title,
    Label,
    Status,
    Dim,
    AccentPullRequest,
    AccentIssue,
    AccentCommit,
    AccentRelease,
    AccentDiscussion,
    Warning,
    Author,
    Count,
    Date,
    PackageList,
}

#[derive(Debug, Clone)]
pub struct PreviewHeaderPart {
    pub text: String,
    pub kind: PreviewHeaderKind,
}

#[derive(Debug, Clone)]
pub struct PreviewHeaderLine {
    pub parts: Vec<PreviewHeaderPart>,
}

#[derive(Debug, Clone)]
pub struct PreviewView {
    pub header: Vec<PreviewHeaderLine>,
    pub body: String,
}

impl PreviewHeaderLine {
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("")
    }
}

impl PreviewView {
    pub fn from(preview: &PreviewData) -> Self {
        use PreviewHeaderKind::{
            AccentCommit, AccentDiscussion, AccentIssue, AccentPullRequest, AccentRelease, Author,
            Count, Date, Dim, Label, PackageList, Status, Title, Warning,
        };

        fn part<T: Into<String>>(text: T, kind: PreviewHeaderKind) -> PreviewHeaderPart {
            PreviewHeaderPart {
                text: text.into(),
                kind,
            }
        }
        let line = |parts: Vec<PreviewHeaderPart>| PreviewHeaderLine { parts };

        let header = match preview {
            PreviewData::PullRequest {
                number,
                title,
                state,
                author,
                comments,
                mergeable,
                ..
            } => vec![
                line(vec![
                    part("PR #", AccentPullRequest),
                    part(number.clone(), AccentPullRequest),
                    part(" - ", Dim),
                    part(title.clone(), Title),
                    part(" [", Dim),
                    part(state.clone(), Status),
                    part("]", Dim),
                ]),
                line(vec![
                    part("Author: ", Label),
                    part(author.clone(), Author),
                    part(" | ", Dim),
                    part("Comments: ", Label),
                    part(comments.to_string(), Count),
                    part(" | ", Dim),
                    part("Mergeable: ", Label),
                    part(mergeable.clone(), Status),
                ]),
            ],
            PreviewData::Issue {
                number,
                title,
                state,
                author,
                comments,
                ..
            } => vec![
                line(vec![
                    part("Issue #", AccentIssue),
                    part(number.clone(), AccentIssue),
                    part(" - ", Dim),
                    part(title.clone(), Title),
                    part(" [", Dim),
                    part(state.clone(), Status),
                    part("]", Dim),
                ]),
                line(vec![
                    part("Author: ", Label),
                    part(author.clone(), Author),
                    part(" | ", Dim),
                    part("Comments: ", Label),
                    part(comments.to_string(), Count),
                ]),
            ],
            PreviewData::Commit { sha, author, .. } => vec![
                line(vec![
                    part("Commit ", AccentCommit),
                    part(sha.chars().take(12).collect::<String>(), AccentCommit),
                ]),
                line(vec![part("Author: ", Label), part(author.clone(), Author)]),
            ],
            PreviewData::Release {
                tag,
                name,
                published_at,
                prerelease,
                ..
            } => vec![
                line(vec![
                    part("Release ", AccentRelease),
                    part(tag.clone(), AccentRelease),
                    part(" - ", Dim),
                    part(name.clone(), Title),
                ]),
                line(vec![
                    part("Published: ", Label),
                    part(published_at.clone(), Date),
                    part(" | ", Dim),
                    part("Pre-release: ", Label),
                    part(if *prerelease { "Yes" } else { "No" }, Status),
                ]),
            ],
            PreviewData::SecurityAlert {
                severity,
                vulnerability_count,
                affected_packages,
                ..
            } => {
                let packages = if affected_packages.is_empty() {
                    "None specified".to_string()
                } else {
                    affected_packages.join(", ")
                };
                vec![
                    line(vec![part("⚠️  ", Warning), part("Security Alert", Warning)]),
                    line(vec![
                        part("Severity: ", Label),
                        part(severity.clone(), Status),
                        part(" | ", Dim),
                        part("Vulnerabilities: ", Label),
                        part(vulnerability_count.to_string(), Count),
                    ]),
                    line(vec![
                        part("Affected Packages: ", Label),
                        part(packages, PackageList),
                    ]),
                ]
            }
            PreviewData::Generic { title, .. } => vec![line(vec![part(title.clone(), Title)])],
            PreviewData::Discussion {
                number,
                title,
                state,
                author,
                comments,
                category,
                answered,
                ..
            } => vec![
                line(vec![
                    part("Discussion #", AccentDiscussion),
                    part(number.clone(), AccentDiscussion),
                    part(" - ", Dim),
                    part(title.clone(), Title),
                    part(" [", Dim),
                    part(state.clone(), Status),
                    part("]", Dim),
                ]),
                line(vec![
                    part("Author: ", Label),
                    part(author.clone(), Author),
                    part(" | ", Dim),
                    part("Category: ", Label),
                    part(category.clone(), Count),
                    part(" | ", Dim),
                    part("Answered: ", Label),
                    part(if *answered { "✓" } else { "✗" }, Status),
                    part(" | ", Dim),
                    part("Comments: ", Label),
                    part(comments.to_string(), Count),
                ]),
            ],
        };

        Self {
            header,
            body: preview.body().to_string(),
        }
    }

    pub fn as_text(&self) -> String {
        let mut lines: Vec<String> = self.header.iter().map(|line| line.text()).collect();
        if !lines.is_empty() {
            lines.push("=".repeat(80));
        }
        lines.push(self.body.clone());
        lines.join("\n")
    }
}

impl PreviewData {
    pub fn body(&self) -> &str {
        match self {
            PreviewData::PullRequest { body, .. } => body,
            PreviewData::Issue { body, .. } => body,
            PreviewData::Commit { body, .. } => body,
            PreviewData::Release { body, .. } => body,
            PreviewData::SecurityAlert { body, .. } => body,
            PreviewData::Discussion { body, .. } => body,
            PreviewData::Generic { body, .. } => body,
        }
    }
}

impl fmt::Display for PreviewData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = PreviewView::from(self);
        write!(f, "{}", view.as_text())
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
                        body: format!(
                            "Release preview unavailable - no URL\n\nRepository: {}",
                            repo_full_name
                        ),
                    }
                }
            }
            NotificationType::Discussion => {
                let number =
                    Self::extract_number_from_url(notification.subject_url(), notification_type)?;
                Self::fetch_discussion_preview(client, repo_full_name, number)?
            }
            NotificationType::RepositoryVulnerabilityAlert => {
                if let Some(url) = notification.subject_url() {
                    Self::fetch_security_alert_preview(client, url)?
                } else {
                    PreviewData::Generic {
                        title: notification.title().to_string(),
                        body: format!(
                            "Security alert preview unavailable - no URL\n\nRepository: {}",
                            repo_full_name
                        ),
                    }
                }
            }
            _ => PreviewData::Generic {
                title: notification.title().to_string(),
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
        let state = pr_state(&pr);
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

        Ok(PreviewData::Commit {
            sha: sha.to_string(),
            author,
            body: message,
        })
    }

    fn fetch_release_by_url(client: &GitHubClient, url: &str) -> Result<PreviewData> {
        // Fetch release by API URL
        // URL format: https://api.github.com/repos/owner/repo/releases/{id}
        let release = client.get_json_by_url(url)?;

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

    fn fetch_discussion_preview(
        client: &GitHubClient,
        repo: &str,
        number: String,
    ) -> Result<PreviewData> {
        let (owner, repo_name) = repo
            .split_once('/')
            .ok_or_else(|| crate::error::Error::Config("Invalid repo format".to_string()))?;

        let discussion_num: i64 = number
            .parse()
            .map_err(|_| crate::error::Error::Config("Invalid discussion number".to_string()))?;

        let query = r#"
            query($owner: String!, $repo: String!, $number: Int!) {
                repository(owner: $owner, name: $repo) {
                    discussion(number: $number) {
                        number
                        title
                        body
                        url
                        stateReason
                        author { login }
                        category { name }
                        answer { id }
                        comments { totalCount }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo_name,
            "number": discussion_num,
        });

        let data = client.graphql(query, variables)?;

        let discussion = data
            .get("repository")
            .and_then(|r| r.get("discussion"))
            .ok_or_else(|| {
                crate::error::Error::Config("Discussion not found in response".to_string())
            })?;

        Self::parse_discussion_response(discussion)
    }

    fn parse_discussion_response(discussion: &serde_json::Value) -> Result<PreviewData> {
        let number = discussion
            .get("number")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .to_string();
        let title = discussion
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("No title")
            .to_string();
        let body = discussion
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("No description")
            .to_string();
        let url = discussion
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let state = discussion
            .get("stateReason")
            .and_then(|v| v.as_str())
            .unwrap_or("OPEN")
            .to_string();
        let author = discussion
            .get("author")
            .and_then(|v| v.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let category = discussion
            .get("category")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("General")
            .to_string();
        let answered = discussion.get("answer").is_some_and(|v| !v.is_null());
        let comments = discussion
            .get("comments")
            .and_then(|v| v.get("totalCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(PreviewData::Discussion {
            number,
            title,
            state,
            author,
            comments,
            category,
            answered,
            body,
            url,
        })
    }
}

/// Derives the display state for a pull request by combining the `state` and
/// `merged` fields from the GitHub API response. The API models these as
/// separate fields — `state` is always `"open"` or `"closed"`, and `merged`
/// is a boolean — so a closed+merged PR must be detected explicitly.
fn pr_state(pr: &serde_json::Value) -> String {
    let merged = pr.get("merged").and_then(|v| v.as_bool()).unwrap_or(false);
    let state_str = if merged {
        "merged"
    } else {
        pr.get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    };
    state_str.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pr_state_open() {
        let pr = json!({"state": "open", "merged": false});
        assert_eq!(pr_state(&pr), "open");
    }

    #[test]
    fn test_pr_state_closed_not_merged() {
        let pr = json!({"state": "closed", "merged": false});
        assert_eq!(pr_state(&pr), "closed");
    }

    #[test]
    fn test_pr_state_merged() {
        let pr = json!({"state": "closed", "merged": true});
        assert_eq!(pr_state(&pr), "merged");
    }

    #[test]
    fn test_pr_state_missing_fields() {
        let pr = json!({});
        assert_eq!(pr_state(&pr), "unknown");
    }

    #[test]
    fn test_parse_discussion_response_full() {
        let discussion = json!({
            "number": 42,
            "title": "How do I configure auth?",
            "body": "I'm trying to set up OAuth and need help.",
            "url": "https://github.com/owner/repo/discussions/42",
            "stateReason": "OPEN",
            "author": { "login": "octocat" },
            "category": { "name": "Q&A" },
            "answer": { "id": "DC_abc123" },
            "comments": { "totalCount": 5 }
        });

        let result = PreviewFetcher::parse_discussion_response(&discussion).unwrap();
        match result {
            PreviewData::Discussion {
                number,
                title,
                state,
                author,
                comments,
                category,
                answered,
                body,
                url,
            } => {
                assert_eq!(number, "42");
                assert_eq!(title, "How do I configure auth?");
                assert_eq!(state, "OPEN");
                assert_eq!(author, "octocat");
                assert_eq!(comments, 5);
                assert_eq!(category, "Q&A");
                assert!(answered);
                assert_eq!(body, "I'm trying to set up OAuth and need help.");
                assert_eq!(url, "https://github.com/owner/repo/discussions/42");
            }
            _ => panic!("Expected PreviewData::Discussion"),
        }
    }

    #[test]
    fn test_parse_discussion_response_unanswered() {
        let discussion = json!({
            "number": 7,
            "title": "Feature request",
            "body": "Please add dark mode",
            "url": "https://github.com/owner/repo/discussions/7",
            "stateReason": "OPEN",
            "author": { "login": "user" },
            "category": { "name": "Ideas" },
            "answer": null,
            "comments": { "totalCount": 0 }
        });

        let result = PreviewFetcher::parse_discussion_response(&discussion).unwrap();
        match result {
            PreviewData::Discussion {
                answered, category, ..
            } => {
                assert!(!answered);
                assert_eq!(category, "Ideas");
            }
            _ => panic!("Expected PreviewData::Discussion"),
        }
    }

    #[test]
    fn test_parse_discussion_response_missing_fields() {
        let discussion = json!({});

        let result = PreviewFetcher::parse_discussion_response(&discussion).unwrap();
        match result {
            PreviewData::Discussion {
                number,
                title,
                state,
                author,
                category,
                answered,
                ..
            } => {
                assert_eq!(number, "0");
                assert_eq!(title, "No title");
                assert_eq!(state, "OPEN");
                assert_eq!(author, "unknown");
                assert_eq!(category, "General");
                assert!(!answered);
            }
            _ => panic!("Expected PreviewData::Discussion"),
        }
    }
}
