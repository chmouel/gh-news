use crate::config::Action;
use crate::models::Notification;
use std::process::{Command, Stdio};

/// Escape a string for safe use in shell commands.
/// Uses single-quote wrapping with proper escaping of embedded single quotes.
fn shell_escape(s: &str) -> String {
    // Replace single quotes with '\'' (end quote, escaped quote, start quote)
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

/// Result of action execution.
pub enum ActionResult {
    /// Action spawned in background.
    Spawned,
    /// Action failed with error message.
    Failed(String),
}

/// Substitute placeholders in a command template with notification data.
///
/// Supported placeholders:
/// - {id} - Notification ID
/// - {title} - Notification title
/// - {url} - Web URL for the notification
/// - {repo} - Repository name (without owner)
/// - {owner} - Repository owner
/// - {full_name} - Full repository name (owner/repo)
/// - {type} - Notification type (Issue, PullRequest, etc.)
/// - {reason} - Notification reason (mention, review_requested, etc.)
/// - {unread} - "true" or "false"
pub fn substitute_placeholders(
    command: &str,
    notification: &Notification,
    github_host: &str,
) -> String {
    let repo_full = notification.repo_full_name();
    let (owner, repo_name) = repo_full.split_once('/').unwrap_or(("", repo_full));

    let url = notification.web_url(github_host).unwrap_or_default();

    // Shell-escape all values to prevent injection attacks
    command
        .replace("{id}", &shell_escape(&notification.id))
        .replace("{title}", &shell_escape(notification.title()))
        .replace("{url}", &shell_escape(&url))
        .replace("{repo}", &shell_escape(repo_name))
        .replace("{owner}", &shell_escape(owner))
        .replace("{full_name}", &shell_escape(repo_full))
        .replace(
            "{type}",
            &shell_escape(&notification.notification_type().to_string()),
        )
        .replace(
            "{reason}",
            &shell_escape(&notification.reason_enum().to_string()),
        )
        .replace("{unread}", &shell_escape(&notification.unread.to_string()))
}

/// Execute an action on a notification (spawns in background).
///
/// For interactive actions, use `prepare_command` instead and execute with terminal access.
pub fn execute_action(
    action: &Action,
    notification: &Notification,
    github_host: &str,
) -> ActionResult {
    let command = substitute_placeholders(&action.command, notification, github_host);

    if command.trim().is_empty() {
        return ActionResult::Failed("Empty command".to_string());
    }

    execute_background(&command)
}

/// Prepare a command string for execution (with placeholder substitution).
/// Used for interactive actions where the caller handles execution.
pub fn prepare_command(action: &Action, notification: &Notification, github_host: &str) -> String {
    substitute_placeholders(&action.command, notification, github_host)
}

/// Execute command in background (non-blocking).
fn execute_background(command: &str) -> ActionResult {
    // Use shell to handle pipes and complex commands
    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match result {
        Ok(_) => ActionResult::Spawned,
        Err(e) => ActionResult::Failed(format!("Failed to spawn: {}", e)),
    }
}

/// Execute a command interactively with full terminal access.
/// Returns Ok(exit_success) or Err with error message.
pub fn execute_interactive(command: &str) -> std::result::Result<bool, String> {
    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(format!("Failed to execute: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NotificationType, Owner, Repository, Subject};

    fn create_test_notification() -> Notification {
        Notification {
            id: "12345".to_string(),
            unread: true,
            last_read_at: None,
            updated_at: None,
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
                title: "Test Issue Title".to_string(),
                subject_type: NotificationType::Issue,
                url: Some("https://api.github.com/repos/chmouel/gh-news/issues/42".to_string()),
                latest_comment_url: None,
            },
            latest_comment_url: None,
        }
    }

    #[test]
    fn test_shell_escape_basic() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn test_shell_escape_with_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_shell_escape_with_special_chars() {
        assert_eq!(shell_escape("$HOME & test | cmd"), "'$HOME & test | cmd'");
    }

    #[test]
    fn test_substitute_basic_placeholders() {
        let notification = create_test_notification();
        let command = "echo {id} {title}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // Values are shell-escaped with single quotes
        assert_eq!(result, "echo '12345' 'Test Issue Title'");
    }

    #[test]
    fn test_substitute_repo_placeholders() {
        let notification = create_test_notification();
        let command = "echo {owner}/{repo} = {full_name}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo 'chmouel'/'gh-news' = 'chmouel/gh-news'");
    }

    #[test]
    fn test_substitute_type_and_reason() {
        let notification = create_test_notification();
        let command = "echo type={type} reason={reason}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo type='Issue' reason='mention'");
    }

    #[test]
    fn test_substitute_unread_status() {
        let notification = create_test_notification();
        let command = "echo unread={unread}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo unread='true'");
    }

    #[test]
    fn test_substitute_url() {
        let notification = create_test_notification();
        let command = "open {url}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert!(result.contains("github.com/chmouel/gh-news/issues/42"));
    }

    #[test]
    fn test_execute_background_command() {
        let action = Action {
            name: "Test".to_string(),
            command: "true".to_string(),
            interactive: false,
        };
        let notification = create_test_notification();
        let result = execute_action(&action, &notification, "github.com");
        assert!(matches!(result, ActionResult::Spawned));
    }

    #[test]
    fn test_prepare_command() {
        let action = Action {
            name: "Test".to_string(),
            command: "echo {id}".to_string(),
            interactive: true,
        };
        let notification = create_test_notification();
        let result = prepare_command(&action, &notification, "github.com");
        assert_eq!(result, "echo '12345'");
    }

    #[test]
    fn test_empty_command_returns_failed() {
        let action = Action {
            name: "Empty".to_string(),
            command: "".to_string(),
            interactive: false,
        };
        let notification = create_test_notification();
        let result = execute_action(&action, &notification, "github.com");
        assert!(matches!(result, ActionResult::Failed(_)));
    }

    #[test]
    fn test_whitespace_only_command_returns_failed() {
        let action = Action {
            name: "Whitespace".to_string(),
            command: "   ".to_string(),
            interactive: false,
        };
        let notification = create_test_notification();
        let result = execute_action(&action, &notification, "github.com");
        assert!(matches!(result, ActionResult::Failed(_)));
    }

    #[test]
    fn test_multiple_placeholders_same_type() {
        let notification = create_test_notification();
        let command = "echo {id} and {id} again";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo '12345' and '12345' again");
    }

    #[test]
    fn test_all_placeholders_in_one_command() {
        let notification = create_test_notification();
        let command = "{id}|{title}|{repo}|{owner}|{full_name}|{type}|{reason}|{unread}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(
            result,
            "'12345'|'Test Issue Title'|'gh-news'|'chmouel'|'chmouel/gh-news'|'Issue'|'mention'|'true'"
        );
    }

    #[test]
    fn test_read_notification_unread_false() {
        let mut notification = create_test_notification();
        notification.unread = false;
        let command = "echo {unread}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo 'false'");
    }

    #[test]
    fn test_pull_request_type() {
        let mut notification = create_test_notification();
        notification.subject.subject_type = NotificationType::PullRequest;
        notification.subject.url =
            Some("https://api.github.com/repos/chmouel/gh-news/pulls/99".to_string());
        let command = "echo {type}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // NotificationType::PullRequest displays as "PR", shell-escaped
        assert_eq!(result, "echo 'PR'");
    }

    #[test]
    fn test_missing_url_falls_back_to_repo_url() {
        let mut notification = create_test_notification();
        notification.subject.url = None;
        let command = "open {url}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // When no subject URL, web_url() falls back to repo URL, shell-escaped
        assert_eq!(result, "open 'https://github.com/chmouel/gh-news'");
    }

    #[test]
    fn test_title_with_special_characters() {
        let mut notification = create_test_notification();
        notification.subject.title = "Fix: handle 'quotes' and \"double quotes\"".to_string();
        let command = "echo {title}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // Single quotes in title are escaped using '\'' pattern
        assert_eq!(
            result,
            "echo 'Fix: handle '\\''quotes'\\'' and \"double quotes\"'"
        );
    }

    #[test]
    fn test_title_with_shell_special_chars() {
        let mut notification = create_test_notification();
        notification.subject.title = "Bug: $HOME variable & pipes | test".to_string();
        let command = "echo {title}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // Shell special chars are safely wrapped in single quotes
        assert_eq!(result, "echo 'Bug: $HOME variable & pipes | test'");
    }

    #[test]
    fn test_pipe_command_placeholder_substitution() {
        let notification = create_test_notification();
        let action = Action {
            name: "PipePlaceholder".to_string(),
            command: "echo {id} | rev".to_string(),
            interactive: false,
        };
        // Verify placeholder substitution works with pipes
        let cmd = prepare_command(&action, &notification, "github.com");
        assert_eq!(cmd, "echo '12345' | rev");
    }

    #[test]
    fn test_github_enterprise_url() {
        let notification = create_test_notification();
        let command = "open {url}";
        let result = substitute_placeholders(command, &notification, "github.example.com");
        assert!(result.contains("github.example.com"));
    }

    #[test]
    fn test_different_reasons() {
        let mut notification = create_test_notification();

        notification.reason = "review_requested".to_string();
        let result = substitute_placeholders("echo {reason}", &notification, "github.com");
        assert_eq!(result, "echo 'review_requested'");

        notification.reason = "assign".to_string();
        let result = substitute_placeholders("echo {reason}", &notification, "github.com");
        assert_eq!(result, "echo 'assign'");

        notification.reason = "ci_activity".to_string();
        let result = substitute_placeholders("echo {reason}", &notification, "github.com");
        assert_eq!(result, "echo 'ci_activity'");
    }

    #[test]
    fn test_no_placeholder_command_unchanged() {
        let notification = create_test_notification();
        let command = "echo hello world";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_unknown_placeholder_left_unchanged() {
        let notification = create_test_notification();
        let command = "echo {unknown} {id}";
        let result = substitute_placeholders(command, &notification, "github.com");
        // Unknown placeholders are left as-is, known ones are shell-escaped
        assert_eq!(result, "echo {unknown} '12345'");
    }

    fn create_pr_notification() -> Notification {
        Notification {
            id: "99999".to_string(),
            unread: false,
            last_read_at: None,
            updated_at: None,
            reason: "review_requested".to_string(),
            repository: Repository {
                id: 2,
                name: "other-repo".to_string(),
                full_name: "org/other-repo".to_string(),
                owner: Owner {
                    login: "org".to_string(),
                    id: 2,
                    owner_type: "Organization".to_string(),
                },
                private: true,
            },
            subject: Subject {
                title: "Add new feature".to_string(),
                subject_type: NotificationType::PullRequest,
                url: Some("https://api.github.com/repos/org/other-repo/pulls/123".to_string()),
                latest_comment_url: None,
            },
            latest_comment_url: None,
        }
    }

    #[test]
    fn test_pr_notification_substitution() {
        let notification = create_pr_notification();
        let command = "{owner}/{repo} PR: {title} ({type})";
        let result = substitute_placeholders(command, &notification, "github.com");
        // NotificationType::PullRequest displays as "PR", all values shell-escaped
        assert_eq!(result, "'org'/'other-repo' PR: 'Add new feature' ('PR')");
    }

    #[test]
    fn test_pr_notification_url() {
        let notification = create_pr_notification();
        let command = "open {url}";
        let result = substitute_placeholders(command, &notification, "github.com");
        assert!(result.contains("github.com/org/other-repo/pull/123"));
    }
}
