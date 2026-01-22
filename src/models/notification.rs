use crate::models::{NotificationReason, NotificationType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub unread: bool,
    #[serde(rename = "last_read_at")]
    pub last_read_at: Option<DateTime<Utc>>,
    #[serde(rename = "updated_at")]
    pub updated_at: Option<DateTime<Utc>>,
    pub reason: String,
    pub repository: crate::models::Repository,
    pub subject: crate::models::Subject,
    #[serde(rename = "latest_comment_url")]
    pub latest_comment_url: Option<String>,
}

impl Notification {
    pub fn is_unread(&self) -> bool {
        self.unread
    }

    pub fn notification_type(&self) -> NotificationType {
        // If the type is Unknown but we have a security_alert reason, infer it's a RepositoryVulnerabilityAlert
        if matches!(self.subject.subject_type, NotificationType::Unknown)
            && matches!(self.reason_enum(), NotificationReason::SecurityAlert)
        {
            return NotificationType::RepositoryVulnerabilityAlert;
        }
        self.subject.subject_type
    }

    pub fn reason_enum(&self) -> NotificationReason {
        self.reason.parse().unwrap_or(NotificationReason::Unknown)
    }

    pub fn repo_full_name(&self) -> &str {
        &self.repository.full_name
    }

    pub fn title(&self) -> &str {
        &self.subject.title
    }

    pub fn subject_url(&self) -> Option<&str> {
        self.subject.url.as_deref()
    }

    pub fn effective_timestamp(&self) -> Option<DateTime<Utc>> {
        if self.unread {
            self.last_read_at.or(self.updated_at)
        } else {
            self.updated_at
        }
    }

    pub fn time_display(&self) -> String {
        use chrono::{Local, TimeZone};

        let time = if self.unread {
            self.last_read_at.or(self.updated_at)
        } else {
            self.updated_at
        };

        let time = match time {
            Some(t) => t,
            None => return "Not available".to_string(),
        };

        let now = Utc::now();
        let duration = now.signed_duration_since(time);
        let hours = duration.num_hours();
        let minutes = duration.num_minutes();

        if hours < 1 {
            format!("{}min ago", minutes)
        } else if hours < 24 {
            format!("{}h ago", hours)
        } else {
            let local_time = Local.from_utc_datetime(&time.naive_utc());
            local_time.format("%d/%b %H:%M").to_string()
        }
    }

    pub fn repo_abbreviated(&self) -> (String, String) {
        fn abbreviate(value: &str, max_chars: usize) -> String {
            let total_chars = value.chars().count();
            if total_chars > max_chars && max_chars > 0 {
                let keep = max_chars.saturating_sub(1);
                let prefix: String = value.chars().take(keep).collect();
                format!("{}…", prefix)
            } else {
                value.to_string()
            }
        }

        let owner = abbreviate(&self.repository.owner.login, 10);
        let name = abbreviate(&self.repository.name, 13);

        (owner, name)
    }

    /// Convert API URL to web URL and return it
    pub fn web_url(&self, github_host: &str) -> Option<String> {
        let api_base = if github_host == "github.com" {
            "https://api.github.com".to_string()
        } else {
            format!("https://{}/api/v3", github_host)
        };
        let web_base = format!("https://{}", github_host);

        let parse_subject_url = |subject_url: &str| -> Option<(String, String, String, String)> {
            let prefix = format!("{}/repos/", api_base);
            if !subject_url.starts_with(&prefix) {
                return None;
            }

            let remainder = subject_url.strip_prefix(&prefix)?;
            let mut parts = remainder.split('/');
            let owner = parts.next()?.to_string();
            let repo = parts.next()?.to_string();
            let resource_type = parts.next()?;
            let number = parts.next()?.to_string();

            let web_type = if resource_type == "pulls" {
                "pull".to_string()
            } else {
                "issues".to_string()
            };

            Some((owner, repo, web_type, number))
        };

        // Prefer latest_comment_url if available (goes directly to the comment)
        if let Some(comment_url) = &self.latest_comment_url {
            // Convert API URL to web URL
            // API: https://api.github.com/repos/owner/repo/issues/comments/123456
            // Web: https://github.com/owner/repo/issues/123#issuecomment-123456
            if let Some(comment_id) = comment_url.split('/').next_back() {
                if let Some(subject_url) = self.subject_url() {
                    // Extract repo and issue/PR number from subject URL
                    // subject_url: https://api.github.com/repos/owner/repo/issues/123
                    // or: https://api.github.com/repos/owner/repo/pulls/456
                    if let Some((owner, repo, web_type, number)) = parse_subject_url(subject_url) {
                        let anchor = if comment_url.contains("/pulls/comments/") {
                            format!("#discussion_r{}", comment_id)
                        } else {
                            format!("#issuecomment-{}", comment_id)
                        };
                        return Some(format!(
                            "{}/{}/{}/{}/{}{}",
                            web_base, owner, repo, web_type, number, anchor
                        ));
                    }
                }
            }
        }

        // Fall back to subject URL conversion
        if let Some(api_url) = self.subject_url() {
            // Convert API URL to web URL
            // API: https://api.github.com/repos/owner/repo/issues/123
            // Web: https://github.com/owner/repo/issues/123
            // API: https://api.github.com/repos/owner/repo/pulls/456
            // Web: https://github.com/owner/repo/pull/456
            // API: https://api.github.com/repos/owner/repo/commits/abc123
            // Web: https://github.com/owner/repo/commit/abc123
            let prefix = format!("{}/repos/", api_base);
            if api_url.starts_with(&prefix) {
                let web_url = api_url
                    .replacen(&prefix, &format!("{}/", web_base), 1)
                    .replace("/pulls/", "/pull/") // PRs use /pull/ not /pulls/
                    .replace("/commits/", "/commit/"); // Commits use /commit/ not /commits/
                return Some(web_url);
            }
            if api_url.starts_with(&web_base) {
                return Some(api_url.to_string());
            }
        }

        // If no URL available, construct a basic repo URL
        Some(format!("{}/{}", web_base, self.repo_full_name()))
    }
}

impl std::str::FromStr for NotificationReason {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "assign" => NotificationReason::Assign,
            "author" => NotificationReason::Author,
            "comment" => NotificationReason::Comment,
            "invitation" => NotificationReason::Invitation,
            "manual" => NotificationReason::Manual,
            "mention" => NotificationReason::Mention,
            "review_requested" => NotificationReason::ReviewRequested,
            "security_alert" => NotificationReason::SecurityAlert,
            "state_change" => NotificationReason::StateChange,
            "subscribed" => NotificationReason::Subscribed,
            "team_mention" => NotificationReason::TeamMention,
            "ci_activity" => NotificationReason::CiActivity,
            _ => NotificationReason::Unknown,
        })
    }
}
