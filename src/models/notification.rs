use crate::models::{NotificationReason, NotificationType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

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
    /// Login of the user who triggered the latest activity on this thread.
    /// Not part of the GitHub API response — populated via background enrichment.
    #[serde(skip)]
    pub author: Option<String>,
    /// Contextual state for state_change notifications (e.g. "merged", "closed", "open",
    /// "closed:completed", "closed:not_planned").
    /// Not part of the GitHub API response — populated via background enrichment.
    #[serde(skip)]
    pub context: Option<String>,
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

    pub fn subject_number(&self) -> Option<String> {
        let url = self.subject.url.as_deref()?;
        let number = url.rsplit('/').next()?;
        if number.chars().all(|c| c.is_ascii_digit()) {
            Some(number.to_string())
        } else {
            None
        }
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

        // "%d/%b %H:%M" produces e.g. "08/Apr 07:50" (12 chars); pad relative times to match.
        const TIME_WIDTH: usize = 12;
        if hours < 1 {
            format!(
                "{:>width$}",
                format!("Today {}m", minutes),
                width = TIME_WIDTH
            )
        } else if hours < 24 {
            format!(
                "{:>width$}",
                format!("Today {}h", hours),
                width = TIME_WIDTH
            )
        } else {
            let local_time = Local.from_utc_datetime(&time.naive_utc());
            local_time.format("%d/%b %H:%M").to_string()
        }
    }

    /// Return a human-readable description combining reason, type, author, and
    /// enriched context — much more informative than the raw API reason string.
    pub fn display_reason(&self) -> String {
        let reason = self.reason_enum();
        let ntype = self.notification_type();

        match reason {
            NotificationReason::StateChange => match self.context.as_deref() {
                Some("merged") => match &self.author {
                    Some(a) => format!("merged by @{a}"),
                    None => "merged".to_string(),
                },
                Some("closed:completed") => "closed as completed".to_string(),
                Some("closed:not_planned") => "closed as not planned".to_string(),
                Some("closed") => "closed".to_string(),
                Some("open") => "reopened".to_string(),
                _ => "state changed".to_string(),
            },
            NotificationReason::Comment => match &self.author {
                Some(a) => format!("@{a} commented"),
                None => "new comment".to_string(),
            },
            NotificationReason::Mention => match &self.author {
                Some(a) => format!("@{a} mentioned you"),
                None => "you were mentioned".to_string(),
            },
            NotificationReason::ReviewRequested => "review requested".to_string(),
            NotificationReason::Author => match ntype {
                NotificationType::PullRequest => "your PR".to_string(),
                NotificationType::Issue => "your issue".to_string(),
                _ => "your thread".to_string(),
            },
            NotificationReason::Assign => "assigned to you".to_string(),
            NotificationReason::Subscribed => "new activity".to_string(),
            NotificationReason::CiActivity => "CI activity".to_string(),
            NotificationReason::SecurityAlert => "security alert".to_string(),
            NotificationReason::TeamMention => "team mentioned".to_string(),
            NotificationReason::ApprovalRequested => "approval requested".to_string(),
            NotificationReason::Invitation => "invitation".to_string(),
            NotificationReason::Manual => "subscribed".to_string(),
            NotificationReason::MemberFeatureRequested => "feature requested".to_string(),
            NotificationReason::SecurityAdvisoryCredit => "security credit".to_string(),
            NotificationReason::Unknown => self.reason.clone(),
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

        // Prefer latest_comment_url if available (goes directly to the comment).
        // Skip for Discussions — the comment URL logic only handles issues/PRs
        // and would incorrectly rewrite to /issues/.
        if !matches!(self.notification_type(), NotificationType::Discussion) {
            if let Some(comment_url) = &self.latest_comment_url {
                // Convert API URL to web URL
                // API: https://api.github.com/repos/owner/repo/issues/comments/123456
                // Web: https://github.com/owner/repo/issues/123#issuecomment-123456
                if let Some(comment_id) = comment_url.split('/').next_back() {
                    if let Some(subject_url) = self.subject_url() {
                        // Extract repo and issue/PR number from subject URL
                        // subject_url: https://api.github.com/repos/owner/repo/issues/123
                        // or: https://api.github.com/repos/owner/repo/pulls/456
                        if let Some((owner, repo, web_type, number)) =
                            parse_subject_url(subject_url)
                        {
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
        }

        // Fall back to subject URL conversion
        if let Some(api_url) = self.subject_url() {
            let prefix = format!("{}/repos/", api_base);
            if api_url.starts_with(&prefix) {
                // Release API URLs use a numeric ID that doesn't work on the web.
                // Use the subject title (which is the tag name) to build the correct URL.
                // API: https://api.github.com/repos/owner/repo/releases/12345
                // Web: https://github.com/owner/repo/releases/tag/v1.0.0
                if let Some(remainder) = api_url.strip_prefix(&prefix) {
                    let mut parts = remainder.split('/');
                    let owner = parts.next();
                    let repo = parts.next();
                    let resource = parts.next();
                    let release_id = parts.next();
                    let trailing = parts.next();

                    if let (Some(owner), Some(repo), Some("releases"), Some(release_id), None) =
                        (owner, repo, resource, release_id, trailing)
                    {
                        if !matches!(self.notification_type(), NotificationType::Release)
                            || release_id.parse::<u64>().is_err()
                        {
                            // Not a release-by-id subject URL; continue with generic conversion below.
                            // This avoids rewriting paths such as /releases/assets/{id}.
                        } else {
                            let mut release_url = Url::parse(&format!(
                                "{}/{}/{}/releases/tag/",
                                web_base, owner, repo
                            ))
                            .ok()?;

                            {
                                let mut path = release_url.path_segments_mut().ok()?;
                                path.pop_if_empty();
                                path.push(&self.subject.title);
                            }

                            return Some(release_url.into());
                        }
                    }
                }

                // Convert API URL to web URL
                // API: https://api.github.com/repos/owner/repo/issues/123
                // Web: https://github.com/owner/repo/issues/123
                // API: https://api.github.com/repos/owner/repo/pulls/456
                // Web: https://github.com/owner/repo/pull/456
                // API: https://api.github.com/repos/owner/repo/commits/abc123
                // Web: https://github.com/owner/repo/commit/abc123
                let web_url = api_url
                    .replacen(&prefix, &format!("{}/", web_base), 1)
                    .replace("/pulls/", "/pull/")
                    .replace("/commits/", "/commit/");
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
        Ok(match s.to_lowercase().as_str() {
            "assign" => NotificationReason::Assign,
            "author" => NotificationReason::Author,
            "comment" => NotificationReason::Comment,
            "invitation" => NotificationReason::Invitation,
            "manual" => NotificationReason::Manual,
            "mention" => NotificationReason::Mention,
            "review_requested" | "reviewrequested" => NotificationReason::ReviewRequested,
            "security_alert" | "securityalert" => NotificationReason::SecurityAlert,
            "state_change" | "statechange" => NotificationReason::StateChange,
            "subscribed" => NotificationReason::Subscribed,
            "team_mention" | "teammention" => NotificationReason::TeamMention,
            "ci_activity" | "ciactivity" => NotificationReason::CiActivity,
            "approval_requested" | "approvalrequested" => NotificationReason::ApprovalRequested,
            "member_feature_requested" | "memberfeaturerequested" => {
                NotificationReason::MemberFeatureRequested
            }
            "security_advisory_credit" | "securityadvisorycredit" => {
                NotificationReason::SecurityAdvisoryCredit
            }
            _ => NotificationReason::Unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Owner, Repository, Subject};

    fn make_notification(subject_url: Option<&str>, title: &str) -> Notification {
        make_notification_with_type(subject_url, title, NotificationType::Release)
    }

    fn make_notification_with_type(
        subject_url: Option<&str>,
        title: &str,
        subject_type: NotificationType,
    ) -> Notification {
        Notification {
            id: "1".to_string(),
            unread: true,
            last_read_at: None,
            updated_at: None,
            reason: "subscribed".to_string(),
            repository: Repository {
                id: 1,
                name: "repo".to_string(),
                full_name: "owner/repo".to_string(),
                owner: Owner {
                    login: "owner".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: title.to_string(),
                subject_type,
                url: subject_url.map(String::from),
                latest_comment_url: None,
            },
            latest_comment_url: None,
            author: None,
            context: None,
        }
    }

    #[test]
    fn web_url_release_uses_tag_name() {
        let n = make_notification(
            Some("https://api.github.com/repos/owner/repo/releases/12345"),
            "v1.2.3",
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/releases/tag/v1.2.3".to_string())
        );
    }

    #[test]
    fn web_url_issue_unchanged() {
        let n = make_notification(
            Some("https://api.github.com/repos/owner/repo/issues/42"),
            "Some issue",
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/issues/42".to_string())
        );
    }

    #[test]
    fn web_url_pull_request_unchanged() {
        let n = make_notification(
            Some("https://api.github.com/repos/owner/repo/pulls/99"),
            "Some PR",
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/pull/99".to_string())
        );
    }

    #[test]
    fn web_url_release_ghes() {
        let n = make_notification(
            Some("https://git.example.com/api/v3/repos/org/proj/releases/789"),
            "v2.0.0",
        );
        assert_eq!(
            n.web_url("git.example.com"),
            Some("https://git.example.com/org/proj/releases/tag/v2.0.0".to_string())
        );
    }

    #[test]
    fn web_url_release_encodes_tag_segment() {
        let n = make_notification(
            Some("https://api.github.com/repos/owner/repo/releases/12345"),
            "release/v1.2.3",
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/releases/tag/release%2Fv1.2.3".to_string())
        );
    }

    #[test]
    fn web_url_release_assets_not_rewritten_to_tag() {
        let n = make_notification(
            Some("https://api.github.com/repos/owner/repo/releases/assets/42"),
            "v1.2.3",
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/releases/assets/42".to_string())
        );
    }

    #[test]
    fn web_url_discussion() {
        let n = make_notification_with_type(
            Some("https://api.github.com/repos/owner/repo/discussions/42"),
            "Some discussion",
            NotificationType::Discussion,
        );
        assert_eq!(
            n.web_url("github.com"),
            Some("https://github.com/owner/repo/discussions/42".to_string())
        );
    }

    #[test]
    fn web_url_discussion_ghes() {
        let n = make_notification_with_type(
            Some("https://git.example.com/api/v3/repos/org/proj/discussions/99"),
            "GHE discussion",
            NotificationType::Discussion,
        );
        assert_eq!(
            n.web_url("git.example.com"),
            Some("https://git.example.com/org/proj/discussions/99".to_string())
        );
    }

    fn make_reason_notification(
        reason: &str,
        subject_type: NotificationType,
        author: Option<&str>,
        context: Option<&str>,
    ) -> Notification {
        let mut n = make_notification_with_type(None, "test", subject_type);
        n.reason = reason.to_string();
        n.author = author.map(String::from);
        n.context = context.map(String::from);
        n
    }

    #[test]
    fn display_reason_state_change_merged_with_author() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::PullRequest,
            Some("alice"),
            Some("merged"),
        );
        assert_eq!(n.display_reason(), "merged by @alice");
    }

    #[test]
    fn display_reason_state_change_merged_no_author() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::PullRequest,
            None,
            Some("merged"),
        );
        assert_eq!(n.display_reason(), "merged");
    }

    #[test]
    fn display_reason_state_change_closed() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::Issue,
            None,
            Some("closed"),
        );
        assert_eq!(n.display_reason(), "closed");
    }

    #[test]
    fn display_reason_state_change_closed_completed() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::Issue,
            None,
            Some("closed:completed"),
        );
        assert_eq!(n.display_reason(), "closed as completed");
    }

    #[test]
    fn display_reason_state_change_closed_not_planned() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::Issue,
            None,
            Some("closed:not_planned"),
        );
        assert_eq!(n.display_reason(), "closed as not planned");
    }

    #[test]
    fn display_reason_state_change_reopened() {
        let n = make_reason_notification(
            "state_change",
            NotificationType::PullRequest,
            None,
            Some("open"),
        );
        assert_eq!(n.display_reason(), "reopened");
    }

    #[test]
    fn display_reason_state_change_no_context() {
        let n = make_reason_notification("state_change", NotificationType::PullRequest, None, None);
        assert_eq!(n.display_reason(), "state changed");
    }

    #[test]
    fn display_reason_comment_with_author() {
        let n = make_reason_notification("comment", NotificationType::Issue, Some("bob"), None);
        assert_eq!(n.display_reason(), "@bob commented");
    }

    #[test]
    fn display_reason_comment_no_author() {
        let n = make_reason_notification("comment", NotificationType::Issue, None, None);
        assert_eq!(n.display_reason(), "new comment");
    }

    #[test]
    fn display_reason_mention_with_author() {
        let n = make_reason_notification(
            "mention",
            NotificationType::PullRequest,
            Some("carol"),
            None,
        );
        assert_eq!(n.display_reason(), "@carol mentioned you");
    }

    #[test]
    fn display_reason_review_requested() {
        let n = make_reason_notification(
            "review_requested",
            NotificationType::PullRequest,
            None,
            None,
        );
        assert_eq!(n.display_reason(), "review requested");
    }

    #[test]
    fn display_reason_author_pr() {
        let n = make_reason_notification("author", NotificationType::PullRequest, None, None);
        assert_eq!(n.display_reason(), "your PR");
    }

    #[test]
    fn display_reason_author_issue() {
        let n = make_reason_notification("author", NotificationType::Issue, None, None);
        assert_eq!(n.display_reason(), "your issue");
    }

    #[test]
    fn display_reason_assign() {
        let n = make_reason_notification("assign", NotificationType::Issue, None, None);
        assert_eq!(n.display_reason(), "assigned to you");
    }

    #[test]
    fn display_reason_subscribed() {
        let n = make_reason_notification("subscribed", NotificationType::PullRequest, None, None);
        assert_eq!(n.display_reason(), "new activity");
    }

    #[test]
    fn display_reason_ci_activity() {
        let n = make_reason_notification("ci_activity", NotificationType::CheckSuite, None, None);
        assert_eq!(n.display_reason(), "CI activity");
    }

    #[test]
    fn display_reason_security_alert() {
        let n = make_reason_notification("security_alert", NotificationType::Unknown, None, None);
        assert_eq!(n.display_reason(), "security alert");
    }
}
