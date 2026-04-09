use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType, Owner, Repository, Subject};
use chrono::{DateTime, Utc};

/// Fetch activity events from the GitHub Events API for the authenticated user.
///
/// When `event_types` is non-empty, only matching event types are included.
pub fn fetch_activity_events(
    client: &GitHubClient,
    event_types: &[String],
) -> Result<Vec<Notification>> {
    let username = client.get_authenticated_user()?;
    let events = client.get_received_events(&username, 30)?;

    let events_array = events.as_array().cloned().unwrap_or_default();

    let mut notifications = Vec::new();
    for event in &events_array {
        let event_type = event
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Filter by event type if configured
        if !event_types.is_empty() && !event_types.iter().any(|t| t == event_type) {
            continue;
        }

        if let Some(notification) = event_to_notification(event, event_type) {
            notifications.push(notification);
        }
    }

    Ok(notifications)
}

fn event_to_notification(event: &serde_json::Value, event_type: &str) -> Option<Notification> {
    let event_id = event.get("id").and_then(|v| v.as_str())?;
    let actor = event
        .get("actor")
        .and_then(|a| a.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let repo_full = event
        .get("repo")
        .and_then(|r| r.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown/unknown");
    let (owner, repo_name) = repo_full.split_once('/').unwrap_or(("unknown", "unknown"));
    let created_at = event
        .get("created_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let title = format_event_title(event_type, actor, repo_full, event);
    let reason = event_type_to_reason(event_type);

    Some(Notification {
        id: format!("event-{}", event_id),
        unread: true,
        last_read_at: None,
        updated_at: created_at,
        reason: reason.to_string(),
        repository: Repository {
            id: 0,
            name: repo_name.to_string(),
            full_name: repo_full.to_string(),
            owner: Owner {
                login: owner.to_string(),
                id: 0,
                owner_type: "User".to_string(),
            },
            private: false,
        },
        subject: Subject {
            title,
            subject_type: NotificationType::ActivityEvent,
            url: None,
            latest_comment_url: None,
        },
        latest_comment_url: None,
        author: None,
    })
}

fn format_event_title(
    event_type: &str,
    actor: &str,
    repo: &str,
    event: &serde_json::Value,
) -> String {
    match event_type {
        "WatchEvent" => format!("{} starred {}", actor, repo),
        "ForkEvent" => format!("{} forked {}", actor, repo),
        "CreateEvent" => {
            let ref_type = event
                .get("payload")
                .and_then(|p| p.get("ref_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("repository");
            let ref_name = event
                .get("payload")
                .and_then(|p| p.get("ref"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if ref_name.is_empty() {
                format!("{} created {} {}", actor, ref_type, repo)
            } else {
                format!("{} created {} {} in {}", actor, ref_type, ref_name, repo)
            }
        }
        "DeleteEvent" => {
            let ref_type = event
                .get("payload")
                .and_then(|p| p.get("ref_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("branch");
            let ref_name = event
                .get("payload")
                .and_then(|p| p.get("ref"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("{} deleted {} {} in {}", actor, ref_type, ref_name, repo)
        }
        "PushEvent" => {
            let size = event
                .get("payload")
                .and_then(|p| p.get("size"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            format!("{} pushed {} commit(s) to {}", actor, size, repo)
        }
        "IssuesEvent" => {
            let action = event
                .get("payload")
                .and_then(|p| p.get("action"))
                .and_then(|v| v.as_str())
                .unwrap_or("updated");
            let issue_title = event
                .get("payload")
                .and_then(|p| p.get("issue"))
                .and_then(|i| i.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("an issue");
            format!("{} {} {}: {}", actor, action, repo, issue_title)
        }
        "PullRequestEvent" => {
            let action = event
                .get("payload")
                .and_then(|p| p.get("action"))
                .and_then(|v| v.as_str())
                .unwrap_or("updated");
            let pr_title = event
                .get("payload")
                .and_then(|p| p.get("pull_request"))
                .and_then(|pr| pr.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("a PR");
            format!("{} {} PR in {}: {}", actor, action, repo, pr_title)
        }
        "ReleaseEvent" => {
            let tag = event
                .get("payload")
                .and_then(|p| p.get("release"))
                .and_then(|r| r.get("tag_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("{} released {} in {}", actor, tag, repo)
        }
        "IssueCommentEvent" => {
            let issue_title = event
                .get("payload")
                .and_then(|p| p.get("issue"))
                .and_then(|i| i.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("an issue");
            format!("{} commented on {}: {}", actor, repo, issue_title)
        }
        "MemberEvent" => {
            let action = event
                .get("payload")
                .and_then(|p| p.get("action"))
                .and_then(|v| v.as_str())
                .unwrap_or("added");
            let member = event
                .get("payload")
                .and_then(|p| p.get("member"))
                .and_then(|m| m.get("login"))
                .and_then(|v| v.as_str())
                .unwrap_or("someone");
            format!("{} {} {} to {}", actor, action, member, repo)
        }
        "PublicEvent" => format!("{} made {} public", actor, repo),
        "GollumEvent" => format!("{} updated wiki pages in {}", actor, repo),
        _ => format!("{}: {} in {}", event_type, actor, repo),
    }
}

fn event_type_to_reason(event_type: &str) -> &'static str {
    match event_type {
        "WatchEvent" | "ForkEvent" | "PublicEvent" => "subscribed",
        "IssuesEvent" | "IssueCommentEvent" => "comment",
        "PullRequestEvent" => "comment",
        "PushEvent" | "CreateEvent" | "DeleteEvent" => "manual",
        "ReleaseEvent" => "manual",
        "MemberEvent" => "invitation",
        _ => "manual",
    }
}
