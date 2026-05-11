use crate::config::{Config, View};
use crate::error::{Error, Result};
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
    patterns: Vec<Regex>,
    exclude_types: Vec<NotificationType>,
    exclude_reasons: Vec<NotificationReason>,
    exclude_repo_patterns: Vec<RepoPattern>,
    exclude_subjects: Vec<Regex>,
}

impl Filter {
    pub fn new(
        pattern: Option<&str>,
        exclude_types: &[String],
        exclude_reasons: &[String],
        exclude_repos: &[String],
        exclude_subjects: &[String],
    ) -> Result<Self> {
        let parsed_types: Vec<NotificationType> = exclude_types
            .iter()
            .map(|s| {
                let parsed: NotificationType = s.parse().unwrap_or(NotificationType::Unknown);
                if parsed == NotificationType::Unknown {
                    Err(Error::Config(format!(
                        "Unknown notification type in exclude_types: {s}"
                    )))
                } else {
                    Ok(parsed)
                }
            })
            .collect::<Result<_>>()?;

        let parsed_reasons: Vec<NotificationReason> = exclude_reasons
            .iter()
            .map(|s| {
                let parsed: NotificationReason = s.parse().unwrap_or(NotificationReason::Unknown);
                if parsed == NotificationReason::Unknown {
                    Err(Error::Config(format!(
                        "Unknown notification reason in exclude_reasons: {s}"
                    )))
                } else {
                    Ok(parsed)
                }
            })
            .collect::<Result<_>>()?;

        let parsed_repos: Vec<RepoPattern> = exclude_repos
            .iter()
            .map(|s| RepoPattern::new(s))
            .collect::<Result<_>>()?;

        let parsed_subjects: Vec<Regex> = exclude_subjects
            .iter()
            .map(|s| Regex::new(&format!("(?i){s}")).map_err(Error::from))
            .collect::<Result<_>>()?;

        let mut patterns = Vec::new();
        if let Some(pattern) = pattern {
            patterns.push(Regex::new(pattern)?);
        }

        Ok(Self {
            patterns,
            exclude_types: parsed_types,
            exclude_reasons: parsed_reasons,
            exclude_repo_patterns: parsed_repos,
            exclude_subjects: parsed_subjects,
        })
    }

    pub fn and(mut self, mut other: Self) -> Self {
        self.patterns.append(&mut other.patterns);
        self.exclude_types.append(&mut other.exclude_types);
        self.exclude_reasons.append(&mut other.exclude_reasons);
        self.exclude_repo_patterns
            .append(&mut other.exclude_repo_patterns);
        self.exclude_subjects.append(&mut other.exclude_subjects);
        self
    }

    /// Create a filter with only a regex pattern (no structured excludes).
    /// Used for interactive search within the TUI.
    pub fn from_pattern(pattern: Option<&str>) -> Result<Self> {
        Self::new(pattern, &[], &[], &[], &[])
    }

    /// Create a filter from a named view, inheriting unset fields from the provided
    /// runtime default pattern and global structured excludes.
    pub fn from_view(view: &View, default_pattern: Option<&str>, config: &Config) -> Result<Self> {
        let pattern = view.filter.as_deref().or(default_pattern);
        let exclude_types = view
            .exclude_types
            .as_deref()
            .unwrap_or(&config.exclude_types);
        let exclude_reasons = view
            .exclude_reasons
            .as_deref()
            .unwrap_or(&config.exclude_reasons);
        let exclude_repos = view
            .exclude_repos
            .as_deref()
            .unwrap_or(&config.exclude_repos);
        let exclude_subjects = view
            .exclude_subjects
            .as_deref()
            .unwrap_or(&config.exclude_subjects);
        Self::new(
            pattern,
            exclude_types,
            exclude_reasons,
            exclude_repos,
            exclude_subjects,
        )
    }

    /// Create a filter from config settings.
    pub fn from_config(pattern: Option<&str>, config: &Config) -> Result<Self> {
        Self::new(
            pattern,
            &config.exclude_types,
            &config.exclude_reasons,
            &config.exclude_repos,
            &config.exclude_subjects,
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
        let title = notification.title();
        if self.exclude_subjects.iter().any(|p| p.is_match(title)) {
            return false;
        }

        // Then apply regex include filter (existing behaviour)
        let text = format!(
            "{} {} {} {} {}",
            notification.repo_full_name(),
            notification.title(),
            notification.notification_type(),
            notification.reason_enum(),
            notification.author.as_deref().unwrap_or("")
        );
        self.patterns.iter().all(|pattern| pattern.is_match(&text))
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
            author: None,
            context: None,
            event_body: None,
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
        let filter = Filter::new(None, &["CheckSuite".to_string()], &[], &[], &[]).unwrap();
        let cs = make_notification("org/repo", "build", "CheckSuite", "subscribed");
        let issue = make_notification("org/repo", "bug", "Issue", "mention");
        assert!(!filter.matches(&cs));
        assert!(filter.matches(&issue));
    }

    #[test]
    fn test_exclude_reason() {
        let filter = Filter::new(None, &[], &["subscribed".to_string()], &[], &[]).unwrap();
        let sub = make_notification("org/repo", "test", "Issue", "subscribed");
        let mention = make_notification("org/repo", "test", "Issue", "mention");
        assert!(!filter.matches(&sub));
        assert!(filter.matches(&mention));
    }

    #[test]
    fn test_exclude_repo_exact() {
        let filter = Filter::new(None, &[], &[], &["org/noisy-repo".to_string()], &[]).unwrap();
        let excluded = make_notification("org/noisy-repo", "test", "Issue", "mention");
        let included = make_notification("org/good-repo", "test", "Issue", "mention");
        assert!(!filter.matches(&excluded));
        assert!(filter.matches(&included));
    }

    #[test]
    fn test_exclude_repo_glob() {
        let filter = Filter::new(None, &[], &[], &["noisy-org/*".to_string()], &[]).unwrap();
        let excluded = make_notification("noisy-org/repo1", "test", "Issue", "mention");
        let also_excluded = make_notification("noisy-org/repo2", "test", "PR", "author");
        let included = make_notification("good-org/repo", "test", "Issue", "mention");
        assert!(!filter.matches(&excluded));
        assert!(!filter.matches(&also_excluded));
        assert!(filter.matches(&included));
    }

    #[test]
    fn test_exclude_combined_with_regex() {
        let filter = Filter::new(Some("bug"), &["CheckSuite".to_string()], &[], &[], &[]).unwrap();
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
    fn test_unrecognised_type_returns_error() {
        let err = Filter::new(None, &["NonExistentType".to_string()], &[], &[], &[]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Unknown notification type in exclude_types: NonExistentType"));
    }

    #[test]
    fn test_unrecognised_reason_returns_error() {
        let err = Filter::new(None, &[], &["mystery".to_string()], &[], &[]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Unknown notification reason in exclude_reasons: mystery"));
    }

    #[test]
    fn test_case_insensitive_type() {
        let filter = Filter::new(
            None,
            &["checksuite".to_string(), "PR".to_string()],
            &[],
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

    #[test]
    fn test_exclude_subject_regex() {
        let filter = Filter::new(
            None,
            &[],
            &[],
            &[],
            &["^Bump ".to_string(), "\\[bot\\]".to_string()],
        )
        .unwrap();
        let bump = make_notification(
            "org/repo",
            "Bump serde from 1.0 to 1.1",
            "PullRequest",
            "subscribed",
        );
        let bot = make_notification("org/repo", "Update deps [bot]", "PullRequest", "subscribed");
        let normal = make_notification("org/repo", "Fix login bug", "Issue", "mention");
        assert!(!filter.matches(&bump));
        assert!(!filter.matches(&bot));
        assert!(filter.matches(&normal));
    }

    #[test]
    fn test_exclude_subject_case_insensitive() {
        let filter = Filter::new(None, &[], &[], &[], &["dependabot".to_string()]).unwrap();
        let upper = make_notification("org/repo", "Dependabot alert", "Issue", "mention");
        let lower = make_notification("org/repo", "dependabot update", "PullRequest", "subscribed");
        let normal = make_notification("org/repo", "Fix something", "Issue", "mention");
        assert!(!filter.matches(&upper));
        assert!(!filter.matches(&lower));
        assert!(filter.matches(&normal));
    }

    #[test]
    fn test_invalid_subject_regex_returns_error() {
        let err = Filter::new(None, &[], &[], &[], &["[".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Filter error"));
    }
}
