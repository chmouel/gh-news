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
        let owner = if self.repository.owner.login.len() > 10 {
            format!("{}…", &self.repository.owner.login[..9])
        } else {
            self.repository.owner.login.clone()
        };

        let name = if self.repository.name.len() > 13 {
            format!("{}…", &self.repository.name[..12])
        } else {
            self.repository.name.clone()
        };

        (owner, name)
    }

    /// Convert API URL to web URL and return it
    pub fn web_url(&self) -> Option<String> {
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
                    let parts: Vec<&str> = subject_url.split('/').collect();
                    if parts.len() >= 8 {
                        let owner = parts[4];
                        let repo = parts[5];
                        let resource_type = parts[6]; // "issues" or "pulls"
                        let number = parts[7];

                        // Determine if it's an issue or PR
                        let web_type = if resource_type == "pulls" {
                            "pull"
                        } else {
                            "issues"
                        };
                        return Some(format!(
                            "https://github.com/{}/{}/{}/{}#issuecomment-{}",
                            owner, repo, web_type, number, comment_id
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
            if api_url.starts_with("https://api.github.com/repos/") {
                let web_url = api_url
                    .replace("https://api.github.com/repos/", "https://github.com/")
                    .replace("/pulls/", "/pull/") // PRs use /pull/ not /pulls/
                    .replace("/commits/", "/commit/"); // Commits use /commit/ not /commits/
                return Some(web_url);
            }
        }

        // If no URL available, construct a basic repo URL
        Some(format!("https://github.com/{}", self.repo_full_name()))
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
