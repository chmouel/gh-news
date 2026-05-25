use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreviewData {
    PullRequest {
        number: String,
        title: String,
        state: String,
        author: String,
        comments: u64,
        mergeable: String,
        body: String,
        labels: Vec<String>,
        review_decision: String,
        is_draft: bool,
        ci_status: String,
        additions: u64,
        deletions: u64,
        changed_files: u64,
    },
    Issue {
        number: String,
        title: String,
        state: String,
        state_reason: String,
        author: String,
        comments: u64,
        body: String,
        labels: Vec<String>,
    },
    Commit {
        sha: String,
        author: String,
        date: String,
        additions: u64,
        deletions: u64,
        changed_files: u64,
        body: String,
    },
    Release {
        tag: String,
        name: String,
        published_at: String,
        prerelease: bool,
        is_draft: bool,
        author: String,
        assets: Vec<String>,
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
        upvotes: u64,
        labels: Vec<String>,
    },
    WorkflowRun {
        name: String,
        run_number: u64,
        status: String,
        conclusion: String,
        branch: String,
        event: String,
        actor: String,
        body: String,
    },
    ActivityEvent {
        event_type: String,
        actor: String,
        repo: String,
        body: String,
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
    Tag,
    AccentWorkflowRun,
    AccentActivityEvent,
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
            AccentActivityEvent, AccentCommit, AccentDiscussion, AccentIssue, AccentPullRequest,
            AccentRelease, AccentWorkflowRun, Author, Count, Date, Dim, Label, PackageList, Status,
            Tag, Title, Warning,
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
                labels,
                review_decision,
                is_draft,
                ci_status,
                additions,
                deletions,
                changed_files,
                ..
            } => {
                let state_text = if *is_draft && state == "open" {
                    "draft".to_string()
                } else {
                    state.clone()
                };
                let mut lines_vec = vec![
                    line(vec![
                        part("PR #", AccentPullRequest),
                        part(number.clone(), AccentPullRequest),
                        part(" - ", Dim),
                        part(title.clone(), Title),
                        part(" [", Dim),
                        part(state_text, Status),
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
                    line(vec![
                        part("Review: ", Label),
                        part(review_decision.clone(), Status),
                        part(" | ", Dim),
                        part("CI: ", Label),
                        part(ci_status.clone(), Status),
                        part(" | ", Dim),
                        part(
                            format!("+{} -{} ({}files)", additions, deletions, changed_files),
                            Count,
                        ),
                    ]),
                ];
                if !labels.is_empty() {
                    lines_vec.push(line(vec![
                        part("Labels: ", Label),
                        part(labels.join(", "), Tag),
                    ]));
                }
                lines_vec
            }
            PreviewData::Issue {
                number,
                title,
                state,
                state_reason,
                author,
                comments,
                labels,
                ..
            } => {
                let display_state = if state_reason.is_empty()
                    || state_reason == "OPEN"
                    || state_reason == "unknown"
                {
                    state.clone()
                } else {
                    format!("{} ({})", state, state_reason.to_lowercase())
                };
                let mut lines_vec = vec![
                    line(vec![
                        part("Issue #", AccentIssue),
                        part(number.clone(), AccentIssue),
                        part(" - ", Dim),
                        part(title.clone(), Title),
                        part(" [", Dim),
                        part(display_state, Status),
                        part("]", Dim),
                    ]),
                    line(vec![
                        part("Author: ", Label),
                        part(author.clone(), Author),
                        part(" | ", Dim),
                        part("Comments: ", Label),
                        part(comments.to_string(), Count),
                    ]),
                ];
                if !labels.is_empty() {
                    lines_vec.push(line(vec![
                        part("Labels: ", Label),
                        part(labels.join(", "), Tag),
                    ]));
                }
                lines_vec
            }
            PreviewData::Commit {
                sha,
                author,
                date,
                additions,
                deletions,
                changed_files,
                ..
            } => {
                let mut lines_vec = vec![
                    line(vec![
                        part("Commit ", AccentCommit),
                        part(sha.chars().take(12).collect::<String>(), AccentCommit),
                    ]),
                    line(vec![part("Author: ", Label), part(author.clone(), Author)]),
                ];
                if !date.is_empty() {
                    lines_vec.push(line(vec![
                        part("Date: ", Label),
                        part(date.clone(), Date),
                        part(" | ", Dim),
                        part(
                            format!("+{} -{} ({}files)", additions, deletions, changed_files),
                            Count,
                        ),
                    ]));
                }
                lines_vec
            }
            PreviewData::Release {
                tag,
                name,
                published_at,
                prerelease,
                is_draft,
                author,
                assets,
                ..
            } => {
                let mut lines_vec = vec![
                    line(vec![
                        part("Release ", AccentRelease),
                        part(tag.clone(), AccentRelease),
                        part(" - ", Dim),
                        part(name.clone(), Title),
                    ]),
                    line(vec![
                        part("Author: ", Label),
                        part(author.clone(), Author),
                        part(" | ", Dim),
                        part("Published: ", Label),
                        part(published_at.clone(), Date),
                        part(" | ", Dim),
                        part("Pre-release: ", Label),
                        part(if *prerelease { "Yes" } else { "No" }, Status),
                        part(" | ", Dim),
                        part("Draft: ", Label),
                        part(if *is_draft { "Yes" } else { "No" }, Status),
                    ]),
                ];
                if !assets.is_empty() {
                    lines_vec.push(line(vec![
                        part("Assets: ", Label),
                        part(assets.join(", "), Tag),
                    ]));
                }
                lines_vec
            }
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
            PreviewData::WorkflowRun {
                name,
                run_number,
                status,
                conclusion,
                branch,
                event,
                actor,
                ..
            } => vec![
                line(vec![
                    part("Workflow ", AccentWorkflowRun),
                    part(name.clone(), AccentWorkflowRun),
                    part(format!(" #{}", run_number), AccentWorkflowRun),
                    part(" [", Dim),
                    part(conclusion.clone(), Status),
                    part("]", Dim),
                ]),
                line(vec![
                    part("Actor: ", Label),
                    part(actor.clone(), Author),
                    part(" | ", Dim),
                    part("Branch: ", Label),
                    part(branch.clone(), Count),
                    part(" | ", Dim),
                    part("Trigger: ", Label),
                    part(event.clone(), Count),
                    part(" | ", Dim),
                    part("Status: ", Label),
                    part(status.clone(), Status),
                ]),
            ],
            PreviewData::ActivityEvent {
                event_type,
                actor,
                repo,
                ..
            } => vec![
                line(vec![
                    part("Event ", AccentActivityEvent),
                    part(event_type.clone(), AccentActivityEvent),
                ]),
                line(vec![
                    part("Actor: ", Label),
                    part(actor.clone(), Author),
                    part(" | ", Dim),
                    part("Repository: ", Label),
                    part(repo.clone(), Count),
                ]),
            ],
            PreviewData::Generic { title, .. } => vec![line(vec![part(title.clone(), Title)])],
            PreviewData::Discussion {
                number,
                title,
                state,
                author,
                comments,
                category,
                answered,
                upvotes,
                labels,
                ..
            } => {
                let mut lines_vec = vec![
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
                        part(if *answered { "Yes" } else { "No" }, Status),
                        part(" | ", Dim),
                        part("Comments: ", Label),
                        part(comments.to_string(), Count),
                        part(" | ", Dim),
                        part("Upvotes: ", Label),
                        part(upvotes.to_string(), Count),
                    ]),
                ];
                if !labels.is_empty() {
                    lines_vec.push(line(vec![
                        part("Labels: ", Label),
                        part(labels.join(", "), Tag),
                    ]));
                }
                lines_vec
            }
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
            PreviewData::WorkflowRun { body, .. } => body,
            PreviewData::ActivityEvent { body, .. } => body,
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

fn extract_label_names(node: &serde_json::Value) -> Vec<String> {
    node.get("labels")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
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
            NotificationType::WorkflowRun => {
                if let Some(url) = notification.subject_url() {
                    Self::fetch_workflow_run_preview(client, url, repo_full_name)?
                } else {
                    PreviewData::Generic {
                        title: notification.title().to_string(),
                        body: format!(
                            "Workflow run preview unavailable - no URL\n\nRepository: {}",
                            repo_full_name
                        ),
                    }
                }
            }
            NotificationType::ActivityEvent => {
                // Activity events are synthetic; use enriched data from the event payload
                let body = notification
                    .event_body
                    .clone()
                    .unwrap_or_else(|| notification.title().to_string());
                PreviewData::ActivityEvent {
                    event_type: notification
                        .context
                        .clone()
                        .unwrap_or_else(|| "Event".to_string()),
                    actor: notification.author.clone().unwrap_or_default(),
                    repo: repo_full_name.to_string(),
                    body,
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
                // For commits, extract full SHA from URL (needed for GraphQL queries)
                // URL format: https://api.github.com/repos/owner/repo/commits/abc123...
                url.split('/')
                    .next_back()
                    .map(|s| s.to_string())
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
        let (owner, repo_name) = repo
            .split_once('/')
            .ok_or_else(|| crate::error::Error::Config("Invalid repo format".to_string()))?;

        let issue_num: i64 = number
            .parse()
            .map_err(|_| crate::error::Error::Config("Invalid issue number".to_string()))?;

        let query = r#"
            query($owner: String!, $repo: String!, $number: Int!) {
                repository(owner: $owner, name: $repo) {
                    issue(number: $number) {
                        number
                        title
                        body
                        state
                        stateReason
                        author { login }
                        comments { totalCount }
                        labels(first: 10) { nodes { name } }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo_name,
            "number": issue_num,
        });

        let data = client.graphql(query, variables)?;

        let issue = data
            .get("repository")
            .and_then(|r| r.get("issue"))
            .filter(|d| !d.is_null())
            .ok_or_else(|| {
                crate::error::Error::Config("Issue not found in response".to_string())
            })?;

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
            .unwrap_or("OPEN")
            .to_lowercase();
        let state_reason = issue
            .get("stateReason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let author = issue
            .get("author")
            .and_then(|v| v.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let comments = issue
            .get("comments")
            .and_then(|v| v.get("totalCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let labels = extract_label_names(issue);

        Ok(PreviewData::Issue {
            number,
            title,
            state,
            state_reason,
            author,
            comments,
            body,
            labels,
        })
    }

    fn fetch_pr_preview(client: &GitHubClient, repo: &str, number: String) -> Result<PreviewData> {
        let (owner, repo_name) = repo
            .split_once('/')
            .ok_or_else(|| crate::error::Error::Config("Invalid repo format".to_string()))?;

        let pr_num: i64 = number
            .parse()
            .map_err(|_| crate::error::Error::Config("Invalid PR number".to_string()))?;

        let query = r#"
            query($owner: String!, $repo: String!, $number: Int!) {
                repository(owner: $owner, name: $repo) {
                    pullRequest(number: $number) {
                        number
                        title
                        body
                        state
                        merged
                        isDraft
                        mergeable
                        reviewDecision
                        additions
                        deletions
                        changedFiles
                        author { login }
                        comments { totalCount }
                        labels(first: 10) { nodes { name } }
                        commits(last: 1) {
                            nodes {
                                commit {
                                    statusCheckRollup { state }
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo_name,
            "number": pr_num,
        });

        let data = client.graphql(query, variables)?;

        let pr = data
            .get("repository")
            .and_then(|r| r.get("pullRequest"))
            .filter(|d| !d.is_null())
            .ok_or_else(|| {
                crate::error::Error::Config("Pull request not found in response".to_string())
            })?;

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
        let merged = pr.get("merged").and_then(|v| v.as_bool()).unwrap_or(false);
        let state = if merged {
            "merged".to_string()
        } else {
            pr.get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("OPEN")
                .to_lowercase()
        };
        let is_draft = pr.get("isDraft").and_then(|v| v.as_bool()).unwrap_or(false);
        let mergeable = match pr.get("mergeable").and_then(|v| v.as_str()) {
            Some("MERGEABLE") => "Yes".to_string(),
            Some("CONFLICTING") => "No".to_string(),
            _ => "Unknown".to_string(),
        };
        let review_decision = pr
            .get("reviewDecision")
            .and_then(|v| v.as_str())
            .unwrap_or("NONE")
            .to_string();
        let author = pr
            .get("author")
            .and_then(|v| v.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let comments = pr
            .get("comments")
            .and_then(|v| v.get("totalCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let additions = pr.get("additions").and_then(|v| v.as_u64()).unwrap_or(0);
        let deletions = pr.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0);
        let changed_files = pr.get("changedFiles").and_then(|v| v.as_u64()).unwrap_or(0);
        let ci_status = pr
            .get("commits")
            .and_then(|v| v.get("nodes"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|n| n.get("commit"))
            .and_then(|c| c.get("statusCheckRollup"))
            .and_then(|s| s.get("state"))
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let labels = extract_label_names(pr);

        Ok(PreviewData::PullRequest {
            number,
            title,
            state,
            author,
            comments,
            mergeable,
            body,
            labels,
            review_decision,
            is_draft,
            ci_status,
            additions,
            deletions,
            changed_files,
        })
    }

    fn fetch_commit_preview(client: &GitHubClient, repo: &str, sha: &str) -> Result<PreviewData> {
        let (owner, repo_name) = repo
            .split_once('/')
            .ok_or_else(|| crate::error::Error::Config("Invalid repo format".to_string()))?;

        // Try GraphQL first for richer data (additions, deletions, changed files)
        if let Ok(preview) = Self::fetch_commit_preview_graphql(client, owner, repo_name, sha) {
            return Ok(preview);
        }

        // Fallback to REST (e.g. GHE without GraphQL)
        let commit = client.get_commit(owner, repo_name, sha)?;

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
            date: String::new(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            body: message,
        })
    }

    fn fetch_commit_preview_graphql(
        client: &GitHubClient,
        owner: &str,
        repo_name: &str,
        sha: &str,
    ) -> Result<PreviewData> {
        let query = r#"
            query($owner: String!, $repo: String!, $oid: GitObjectID!) {
                repository(owner: $owner, name: $repo) {
                    object(oid: $oid) {
                        ... on Commit {
                            oid
                            message
                            author {
                                name
                                user { login }
                                date
                            }
                            additions
                            deletions
                            changedFilesIfAvailable
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo_name,
            "oid": sha,
        });

        let data = client.graphql(query, variables)?;

        let commit = data
            .get("repository")
            .and_then(|r| r.get("object"))
            .filter(|d| !d.is_null())
            .ok_or_else(|| {
                crate::error::Error::Config("Commit not found in response".to_string())
            })?;

        let message = commit
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("No message")
            .to_string();
        let author = commit
            .get("author")
            .and_then(|a| {
                a.get("user")
                    .and_then(|u| u.get("login"))
                    .and_then(|v| v.as_str())
                    .or_else(|| a.get("name").and_then(|v| v.as_str()))
            })
            .unwrap_or("unknown")
            .to_string();
        let date = commit
            .get("author")
            .and_then(|a| a.get("date"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let additions = commit
            .get("additions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let deletions = commit
            .get("deletions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let changed_files = commit
            .get("changedFilesIfAvailable")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(PreviewData::Commit {
            sha: sha.to_string(),
            author,
            date,
            additions,
            deletions,
            changed_files,
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
        let is_draft = release
            .get("draft")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let author = release
            .get("author")
            .and_then(|a| a.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let assets = release
            .get("assets")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(PreviewData::Release {
            tag,
            name,
            published_at,
            prerelease,
            is_draft,
            author,
            assets,
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
                        upvoteCount
                        labels(first: 10) { nodes { name } }
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
            .filter(|d| !d.is_null())
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
        let upvotes = discussion
            .get("upvoteCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let labels = extract_label_names(discussion);

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
            upvotes,
            labels,
        })
    }

    fn fetch_workflow_run_preview(
        client: &GitHubClient,
        url: &str,
        repo: &str,
    ) -> Result<PreviewData> {
        // For workflow runs, the URL may be an HTML URL rather than an API URL.
        // Try to extract run_id and fetch via API.
        let (owner, repo_name) = repo
            .split_once('/')
            .ok_or_else(|| crate::error::Error::Config("Invalid repo format".to_string()))?;

        // Extract run ID from URL (last numeric segment)
        let run_id = url
            .split('/')
            .rev()
            .find(|s| s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty())
            .unwrap_or("");

        if run_id.is_empty() {
            return Ok(PreviewData::Generic {
                title: "Workflow Run".to_string(),
                body: format!("Could not extract run ID from URL: {}", url),
            });
        }

        let api_url = format!(
            "{}/repos/{}/{}/actions/runs/{}",
            client.api_base(),
            owner,
            repo_name,
            run_id
        );
        let run = client.get_json_by_url(&api_url)?;

        let name = run
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let run_number = run.get("run_number").and_then(|v| v.as_u64()).unwrap_or(0);
        let status = run
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let conclusion = run
            .get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let branch = run
            .get("head_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let event = run
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let actor = run
            .get("actor")
            .and_then(|a| a.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Fetch jobs for the body
        let jobs_url = format!("{}/jobs", api_url);
        let body = match client.get_json_by_url(&jobs_url) {
            Ok(jobs_data) => {
                let jobs = jobs_data
                    .get("jobs")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                jobs.iter()
                    .map(|job| {
                        let job_name = job.get("name").and_then(|v| v.as_str()).unwrap_or("Job");
                        let job_conclusion = job
                            .get("conclusion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        format!("- {} [{}]", job_name, job_conclusion)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Err(_) => format!("Workflow: {} #{}", name, run_number),
        };

        Ok(PreviewData::WorkflowRun {
            name,
            run_number,
            status,
            conclusion,
            branch,
            event,
            actor,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_label_names() {
        let node = json!({
            "labels": { "nodes": [{ "name": "bug" }, { "name": "urgent" }] }
        });
        assert_eq!(extract_label_names(&node), vec!["bug", "urgent"]);

        let empty = json!({});
        assert!(extract_label_names(&empty).is_empty());
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
            "comments": { "totalCount": 5 },
            "upvoteCount": 3,
            "labels": { "nodes": [{ "name": "help wanted" }] }
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
                upvotes,
                labels,
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
                assert_eq!(upvotes, 3);
                assert_eq!(labels, vec!["help wanted"]);
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
                upvotes,
                labels,
                ..
            } => {
                assert_eq!(number, "0");
                assert_eq!(title, "No title");
                assert_eq!(state, "OPEN");
                assert_eq!(author, "unknown");
                assert_eq!(category, "General");
                assert!(!answered);
                assert_eq!(upvotes, 0);
                assert!(labels.is_empty());
            }
            _ => panic!("Expected PreviewData::Discussion"),
        }
    }

    #[test]
    fn test_preview_view_displays_draft_only_for_open_pull_requests() {
        let open_draft = PreviewData::PullRequest {
            number: "1".to_string(),
            title: "Open draft".to_string(),
            state: "open".to_string(),
            author: "octocat".to_string(),
            comments: 0,
            mergeable: "Unknown".to_string(),
            body: String::new(),
            labels: Vec::new(),
            review_decision: "NONE".to_string(),
            is_draft: true,
            ci_status: "UNKNOWN".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
        };
        let closed_draft = PreviewData::PullRequest {
            number: "2".to_string(),
            title: "Closed draft".to_string(),
            state: "closed".to_string(),
            author: "octocat".to_string(),
            comments: 0,
            mergeable: "Unknown".to_string(),
            body: String::new(),
            labels: Vec::new(),
            review_decision: "NONE".to_string(),
            is_draft: true,
            ci_status: "UNKNOWN".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
        };
        let merged_draft = PreviewData::PullRequest {
            number: "3".to_string(),
            title: "Merged draft".to_string(),
            state: "merged".to_string(),
            author: "octocat".to_string(),
            comments: 0,
            mergeable: "Unknown".to_string(),
            body: String::new(),
            labels: Vec::new(),
            review_decision: "NONE".to_string(),
            is_draft: true,
            ci_status: "UNKNOWN".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
        };

        let open_header = PreviewView::from(&open_draft).header[0].text();
        let closed_header = PreviewView::from(&closed_draft).header[0].text();
        let merged_header = PreviewView::from(&merged_draft).header[0].text();

        assert!(open_header.ends_with("[draft]"));
        assert!(closed_header.ends_with("[closed]"));
        assert!(merged_header.ends_with("[merged]"));
    }
}
