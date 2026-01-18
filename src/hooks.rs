use crate::models::Notification;
use std::process::{Command, Stdio};

/// Execute a user-configured command for a new notification.
///
/// The command is spawned with environment variables containing notification metadata.
/// This is non-blocking - the process is spawned and immediately forgotten.
///
/// # Environment Variables Set
/// - GH_NEWS_ID
/// - GH_NEWS_TITLE
/// - GH_NEWS_REPO
/// - GH_NEWS_OWNER
/// - GH_NEWS_TYPE
/// - GH_NEWS_REASON
/// - GH_NEWS_UNREAD
/// - GH_NEWS_URL (if available)
/// - GH_NEWS_UPDATED_AT (if available)
///
/// # Errors
/// Returns Err if the command fails to spawn (e.g., command not found).
pub fn execute_new_notification_hook(
    command_template: &str,
    notification: &Notification,
) -> std::io::Result<()> {
    // Parse command into program and arguments
    let parts: Vec<&str> = command_template.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(()); // Empty command, no-op
    }

    let (program, args) = parts.split_first().unwrap();

    // Extract owner from repo full name (format: "owner/repo")
    let repo_full = notification.repo_full_name();
    let (owner, repo_name) = repo_full.split_once('/').unwrap_or(("", repo_full));

    // Build command with environment variables
    let mut child = Command::new(program);
    child
        .args(args)
        .env("GH_NEWS_ID", &notification.id)
        .env("GH_NEWS_TITLE", notification.title())
        .env("GH_NEWS_REPO", repo_name)
        .env("GH_NEWS_OWNER", owner)
        .env(
            "GH_NEWS_TYPE",
            notification.notification_type().to_string(),
        )
        .env("GH_NEWS_REASON", notification.reason_enum().to_string())
        .env("GH_NEWS_UNREAD", notification.unread.to_string());

    // Add optional fields
    if let Some(url) = notification.web_url() {
        child.env("GH_NEWS_URL", url);
    }

    if let Some(updated_at) = notification.updated_at {
        child.env("GH_NEWS_UPDATED_AT", updated_at.to_rfc3339());
    }

    // Redirect stdout/stderr to avoid polluting TUI output
    child.stdout(Stdio::null()).stderr(Stdio::null());

    // Spawn and forget (non-blocking)
    child.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_notification() -> Notification {
        use crate::models::{NotificationType, Owner, Repository, Subject};
        use chrono::Utc;

        Notification {
            id: "test-id-123".to_string(),
            unread: true,
            last_read_at: None,
            updated_at: Some(Utc::now()),
            reason: "mention".to_string(),
            repository: Repository {
                id: 1,
                name: "gh-news".to_string(),
                full_name: "chmouel/gh-news".to_string(),
                owner: Owner {
                    login: "chmouel".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: Subject {
                title: "Test Notification".to_string(),
                subject_type: NotificationType::Issue,
                url: Some("https://api.github.com/repos/chmouel/gh-news/issues/1".to_string()),
                latest_comment_url: None,
            },
            latest_comment_url: None,
        }
    }

    #[test]
    fn test_empty_command_is_noop() {
        let notification = create_test_notification();
        let result = execute_new_notification_hook("", &notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_command_spawns() {
        let notification = create_test_notification();
        // Use "true" command which exists on Unix systems and always succeeds
        let result = execute_new_notification_hook("true", &notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_command_returns_error() {
        let notification = create_test_notification();
        let result = execute_new_notification_hook(
            "this_command_definitely_does_not_exist_xyz",
            &notification,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_command_with_args() {
        let notification = create_test_notification();
        // "echo" with arguments should spawn successfully
        let result = execute_new_notification_hook("echo test", &notification);
        assert!(result.is_ok());
    }
}
