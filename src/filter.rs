use crate::config::Config;
use crate::error::Result;
use crate::models::enums::{NotificationReason, NotificationType};
use crate::models::Notification;
use regex::Regex;

#[derive(Debug, Clone)]
enum RepoPattern {
    Exact(String),
    Glob(Regex),
}

impl RepoPattern {
    fn new(pattern: &str) -> Result<Self> {
        if pattern.contains('*') || pattern.contains('?') {
            let regex_str = format!(
                "^{}$",
                regex::escape(pattern)
                    .replace(r"\*", ".*")
                    .replace(r"\?", ".")
            );
            Ok(RepoPattern::Glob(Regex::new(&regex_str)?))
        } else {
            Ok(RepoPattern::Exact(pattern.to_lowercase()))
        }
    }

    fn matches(&self, repo: &str) -> bool {
        match self {
            RepoPattern::Exact(name) => repo.to_lowercase() == *name,
            RepoPattern::Glob(regex) => regex.is_match(repo),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Filter {
    pattern: Option<Regex>,
    exclude_types: Vec<NotificationType>,
    exclude_reasons: Vec<NotificationReason>,
    exclude_repo_patterns: Vec<RepoPattern>,
}

impl Filter {
    pub fn new(
        pattern: Option<&str>,
        exclude_types: &[String],
        exclude_reasons: &[String],
        exclude_repos: &[String],
    ) -> Result<Self> {
        let parsed_types: Vec<NotificationType> = exclude_types
            .iter()
            .filter_map(|s| {
                let t: NotificationType = s.parse().ok()?;
                if t == NotificationType::Unknown {
                    None
                } else {
                    Some(t)
                }
            })
            .collect();

        let parsed_reasons: Vec<NotificationReason> = exclude_reasons
            .iter()
            .filter_map(|s| {
                let r: NotificationReason = s.parse().ok()?;
                if r == NotificationReason::Unknown {
                    None
                } else {
                    Some(r)
                }
            })
            .collect();

        let parsed_repos: Vec<RepoPattern> = exclude_repos
            .iter()
            .filter_map(|s| RepoPattern::new(s).ok())
            .collect();

        Ok(Self {
            pattern: match pattern {
                Some(p) => Some(Regex::new(p)?),
                None => None,
            },
            exclude_types: parsed_types,
            exclude_reasons: parsed_reasons,
            exclude_repo_patterns: parsed_repos,
        })
    }

    /// Create a filter with only a regex pattern (no structured excludes).
    /// Used for interactive search within the TUI.
    pub fn from_pattern(pattern: Option<&str>) -> Result<Self> {
        Self::new(pattern, &[], &[], &[])
    }

    /// Create a filter from config settings.
    pub fn from_config(pattern: Option<&str>, config: &Config) -> Result<Self> {
        Self::new(
            pattern,
            &config.exclude_types,
            &config.exclude_reasons,
            &config.exclude_repos,
        )
    }

    pub fn matches(&self, notification: &Notification) -> bool {
        // Check structured excludes first
        if self
            .exclude_types
            .contains(&notification.notification_type())
        {
            return false;
        }
        if self.exclude_reasons.contains(&notification.reason_enum()) {
            return false;
        }
        let repo = notification.repo_full_name();
        if self.exclude_repo_patterns.iter().any(|p| p.matches(repo)) {
            return false;
        }

        // Then apply regex include filter (existing behaviour)
        if let Some(ref pattern) = self.pattern {
            let text = format!(
                "{} {} {} {}",
                notification.repo_full_name(),
                notification.title(),
                notification.notification_type(),
                notification.reason_enum()
            );
            pattern.is_match(&text)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Owner, Repository, Subject};

    fn make_notification(repo: &str, title: &str, ntype: &str, reason: &str) -> Notification {
        let parts: Vec<&str> = repo.splitn(2, '/').collect();
        let (owner, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("owner", repo)
        };
        let subject_type: NotificationType = ntype.parse().unwrap_or(NotificationType::Unknown);
        Notification {
            id: "1".to_string(),
            unread: true,
            last_read_at: None,
            updated_at: None,
            reason: reason.to_string(),
            repository: Repository {
                id: 1,
                full_name: repo.to_string(),
                name: name.to_string(),
                owner: Owner {
                    login: owner.to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: title.to_string(),
                url: None,
                subject_type,
                latest_comment_url: None,
            },
            latest_comment_url: None,
        }
    }

    #[test]
    fn test_no_filters_matches_all() {
        let filter = Filter::from_pattern(None).unwrap();
        let n = make_notification("org/repo", "test", "Issue", "mention");
        assert!(filter.matches(&n));
    }

    #[test]
    fn test_exclude_type() {
        let filter = Filter::new(None, &["CheckSuite".to_string()], &[], &[]).unwrap();
        let cs = make_notification("org/repo", "build", "CheckSuite", "subscribed");
        let issue = make_notification("org/repo", "bug", "Issue", "mention");
        assert!(!filter.matches(&cs));
        assert!(filter.matches(&issue));
    }

    #[test]
    fn test_exclude_reason() {
        let filter = Filter::new(None, &[], &["subscribed".to_string()], &[]).unwrap();
        let sub = make_notification("org/repo", "test", "Issue", "subscribed");
        let mention = make_notification("org/repo", "test", "Issue", "mention");
        assert!(!filter.matches(&sub));
        assert!(filter.matches(&mention));
    }

    #[test]
    fn test_exclude_repo_exact() {
        let filter = Filter::new(None, &[], &[], &["org/noisy-repo".to_string()]).unwrap();
        let excluded = make_notification("org/noisy-repo", "test", "Issue", "mention");
        let included = make_notification("org/good-repo", "test", "Issue", "mention");
        assert!(!filter.matches(&excluded));
        assert!(filter.matches(&included));
    }

    #[test]
    fn test_exclude_repo_glob() {
        let filter = Filter::new(None, &[], &[], &["noisy-org/*".to_string()]).unwrap();
        let excluded = make_notification("noisy-org/repo1", "test", "Issue", "mention");
        let also_excluded = make_notification("noisy-org/repo2", "test", "PR", "author");
        let included = make_notification("good-org/repo", "test", "Issue", "mention");
        assert!(!filter.matches(&excluded));
        assert!(!filter.matches(&also_excluded));
        assert!(filter.matches(&included));
    }

    #[test]
    fn test_exclude_combined_with_regex() {
        let filter = Filter::new(Some("bug"), &["CheckSuite".to_string()], &[], &[]).unwrap();
        // Matches regex but excluded by type
        let cs = make_notification("org/repo", "bug in checksuite", "CheckSuite", "mention");
        // Matches regex and not excluded
        let issue = make_notification("org/repo", "bug report", "Issue", "mention");
        // Does not match regex
        let feat = make_notification("org/repo", "feature", "Issue", "mention");
        assert!(!filter.matches(&cs));
        assert!(filter.matches(&issue));
        assert!(!filter.matches(&feat));
    }

    #[test]
    fn test_unrecognised_type_ignored() {
        let filter = Filter::new(
            None,
            &["NonExistentType".to_string(), "Issue".to_string()],
            &[],
            &[],
        )
        .unwrap();
        // Only Issue should be excluded, unrecognised type silently ignored
        assert_eq!(filter.exclude_types.len(), 1);
        assert_eq!(filter.exclude_types[0], NotificationType::Issue);
    }

    #[test]
    fn test_case_insensitive_type() {
        let filter = Filter::new(
            None,
            &["checksuite".to_string(), "PR".to_string()],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(filter.exclude_types.len(), 2);
        assert!(filter.exclude_types.contains(&NotificationType::CheckSuite));
        assert!(filter
            .exclude_types
            .contains(&NotificationType::PullRequest));
    }

    #[test]
    fn test_from_config() {
        let config = Config {
            exclude_types: vec!["Release".to_string()],
            exclude_reasons: vec!["ci_activity".to_string()],
            exclude_repos: vec!["bot-org/*".to_string()],
            ..Config::default()
        };
        let filter = Filter::from_config(None, &config).unwrap();
        let release = make_notification("org/repo", "v1.0", "Release", "mention");
        let ci = make_notification("org/repo", "build", "Issue", "ci_activity");
        let bot = make_notification("bot-org/spam", "update", "Issue", "mention");
        let normal = make_notification("org/repo", "fix", "Issue", "mention");
        assert!(!filter.matches(&release));
        assert!(!filter.matches(&ci));
        assert!(!filter.matches(&bot));
        assert!(filter.matches(&normal));
    }
}
