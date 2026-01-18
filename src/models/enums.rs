use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationReason {
    Assign,
    Author,
    Comment,
    Invitation,
    Manual,
    Mention,
    ReviewRequested,
    SecurityAlert,
    StateChange,
    Subscribed,
    TeamMention,
    CiActivity,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NotificationType {
    Issue,
    PullRequest,
    Discussion,
    Commit,
    Release,
    CheckSuite,
    RepositoryVulnerabilityAlert,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for NotificationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationReason::Assign => write!(f, "assign"),
            NotificationReason::Author => write!(f, "author"),
            NotificationReason::Comment => write!(f, "comment"),
            NotificationReason::Invitation => write!(f, "invitation"),
            NotificationReason::Manual => write!(f, "manual"),
            NotificationReason::Mention => write!(f, "mention"),
            NotificationReason::ReviewRequested => write!(f, "review_requested"),
            NotificationReason::SecurityAlert => write!(f, "security_alert"),
            NotificationReason::StateChange => write!(f, "state_change"),
            NotificationReason::Subscribed => write!(f, "subscribed"),
            NotificationReason::TeamMention => write!(f, "team_mention"),
            NotificationReason::CiActivity => write!(f, "ci_activity"),
            NotificationReason::Unknown => write!(f, "unknown"),
        }
    }
}

impl std::fmt::Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationType::Issue => write!(f, "Issue"),
            NotificationType::PullRequest => write!(f, "PR"),
            NotificationType::Discussion => write!(f, "Discussion"),
            NotificationType::Commit => write!(f, "Commit"),
            NotificationType::Release => write!(f, "Release"),
            NotificationType::CheckSuite => write!(f, "CheckSuite"),
            NotificationType::RepositoryVulnerabilityAlert => {
                write!(f, "Security")
            }
            NotificationType::Unknown => write!(f, "Unknown"),
        }
    }
}
