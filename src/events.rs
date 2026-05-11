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

        if let Some(notification) = event_to_notification(event, event_type, "event-") {
            notifications.push(notification);
        }
    }

    enrich_event_titles(client, &mut notifications);

    Ok(notifications)
}

pub(crate) fn event_to_notification(
    event: &serde_json::Value,
    event_type: &str,
    id_prefix: &str,
) -> Option<Notification> {
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
    let subject_url = extract_subject_url(event_type, event);
    let event_body = extract_event_body(event_type, event);

    Some(Notification {
        id: format!("{}{}", id_prefix, event_id),
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
            url: subject_url,
            latest_comment_url: None,
        },
        latest_comment_url: None,
        author: Some(actor.to_string()),
        context: Some(event_type.to_string()),
        event_body,
    })
}

/// Fetch events from watched repositories via `GET /repos/{owner}/{repo}/events`.
///
/// When `event_types` is non-empty, only matching event types are included.
pub fn fetch_watch_repo_events(
    client: &GitHubClient,
    repos: &[String],
    event_types: &[String],
) -> Result<Vec<Notification>> {
    let mut all_notifications = Vec::new();

    for repo_full in repos {
        let (owner, repo_name) = match repo_full.split_once('/') {
            Some(parts) => parts,
            None => continue,
        };

        let events = match client.get_repo_events(owner, repo_name, 30) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let events_array = events.as_array().cloned().unwrap_or_default();

        for event in &events_array {
            let event_type = event
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            if !event_types.is_empty() && !event_types.iter().any(|t| t == event_type) {
                continue;
            }

            if let Some(notification) = event_to_notification(event, event_type, "repo-event-") {
                all_notifications.push(notification);
            }
        }
    }

    all_notifications.sort_by_key(|n| std::cmp::Reverse(n.updated_at));

    enrich_event_titles(client, &mut all_notifications);

    Ok(all_notifications)
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
            let payload = event.get("payload");
            let action = payload
                .and_then(|p| p.get("action"))
                .and_then(|v| v.as_str())
                .unwrap_or("updated");
            let issue = payload.and_then(|p| p.get("issue"));
            let issue_title = issue.and_then(|i| i.get("title")).and_then(|v| v.as_str());
            let number = issue.and_then(|i| i.get("number")).and_then(|v| v.as_u64());
            let action_detail = format_action_detail(action, payload);
            let num_str = number.map(|n| format!("#{n}")).unwrap_or_default();
            let suffix = issue_title.map(|t| format!(": {t}")).unwrap_or_default();
            format!(
                "{} {} issue {} in {}{}",
                actor, action_detail, num_str, repo, suffix
            )
        }
        "PullRequestEvent" => {
            let payload = event.get("payload");
            let action = payload
                .and_then(|p| p.get("action"))
                .and_then(|v| v.as_str())
                .unwrap_or("updated");
            let pr = payload.and_then(|p| p.get("pull_request"));
            let pr_title = pr.and_then(|p| p.get("title")).and_then(|v| v.as_str());
            let number = pr.and_then(|p| p.get("number")).and_then(|v| v.as_u64());
            let action_detail = format_action_detail(action, payload);
            let num_str = number.map(|n| format!("#{n}")).unwrap_or_default();
            let suffix = pr_title.map(|t| format!(": {t}")).unwrap_or_default();
            format!(
                "{} {} PR {} in {}{}",
                actor, action_detail, num_str, repo, suffix
            )
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
            let issue = event.get("payload").and_then(|p| p.get("issue"));
            let issue_title = issue
                .and_then(|i| i.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("an issue");
            let number = issue.and_then(|i| i.get("number")).and_then(|v| v.as_u64());
            match number {
                Some(n) => format!("{} commented on #{} in {}: {}", actor, n, repo, issue_title),
                None => format!("{} commented on {}: {}", actor, repo, issue_title),
            }
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

/// Enrich the action string with contextual detail (label name, assignee login).
fn format_action_detail(action: &str, payload: Option<&serde_json::Value>) -> String {
    match action {
        "labeled" | "unlabeled" => {
            let label = payload
                .and_then(|p| p.get("label"))
                .and_then(|l| l.get("name"))
                .and_then(|v| v.as_str());
            match label {
                Some(name) => format!("{action} \"{name}\" on"),
                None => action.to_string(),
            }
        }
        "assigned" | "unassigned" => {
            let assignee = payload
                .and_then(|p| p.get("assignee"))
                .and_then(|a| a.get("login"))
                .and_then(|v| v.as_str());
            match assignee {
                Some(login) => format!("{action} @{login} on"),
                None => action.to_string(),
            }
        }
        _ => action.to_string(),
    }
}

fn extract_subject_url(event_type: &str, event: &serde_json::Value) -> Option<String> {
    let payload = event.get("payload")?;
    match event_type {
        "PullRequestEvent" => payload
            .get("pull_request")?
            .get("url")?
            .as_str()
            .map(String::from),
        "IssuesEvent" => payload.get("issue")?.get("url")?.as_str().map(String::from),
        "IssueCommentEvent" => payload.get("issue")?.get("url")?.as_str().map(String::from),
        "ReleaseEvent" => payload
            .get("release")?
            .get("url")?
            .as_str()
            .map(String::from),
        _ => None,
    }
}

fn extract_event_body(event_type: &str, event: &serde_json::Value) -> Option<String> {
    let payload = event.get("payload")?;
    let action = payload.get("action").and_then(|v| v.as_str());

    match event_type {
        "PullRequestEvent" => {
            let pr = payload.get("pull_request")?;
            let pr_body = pr.get("body").and_then(|v| v.as_str());
            match action {
                Some("labeled" | "unlabeled") => {
                    let label = payload
                        .get("label")
                        .and_then(|l| l.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let desc = pr_body.unwrap_or("");
                    Some(format!("Label: {label}\n\n{desc}").trim().to_string())
                }
                Some("assigned" | "unassigned") => {
                    let assignee = payload
                        .get("assignee")
                        .and_then(|a| a.get("login"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let desc = pr_body.unwrap_or("");
                    Some(
                        format!("Assignee: @{assignee}\n\n{desc}")
                            .trim()
                            .to_string(),
                    )
                }
                _ => pr_body.map(String::from),
            }
        }
        "IssuesEvent" => {
            let issue = payload.get("issue")?;
            let issue_body = issue.get("body").and_then(|v| v.as_str());
            match action {
                Some("labeled" | "unlabeled") => {
                    let label = payload
                        .get("label")
                        .and_then(|l| l.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let desc = issue_body.unwrap_or("");
                    Some(format!("Label: {label}\n\n{desc}").trim().to_string())
                }
                Some("assigned" | "unassigned") => {
                    let assignee = payload
                        .get("assignee")
                        .and_then(|a| a.get("login"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let desc = issue_body.unwrap_or("");
                    Some(
                        format!("Assignee: @{assignee}\n\n{desc}")
                            .trim()
                            .to_string(),
                    )
                }
                _ => issue_body.map(String::from),
            }
        }
        "IssueCommentEvent" => payload
            .get("comment")?
            .get("body")?
            .as_str()
            .map(String::from),
        "PushEvent" => {
            let commits = payload.get("commits")?.as_array()?;
            let messages: Vec<&str> = commits
                .iter()
                .filter_map(|c| c.get("message").and_then(|m| m.as_str()))
                .collect();
            if messages.is_empty() {
                None
            } else {
                Some(messages.join("\n\n"))
            }
        }
        "ReleaseEvent" => payload
            .get("release")?
            .get("body")?
            .as_str()
            .map(String::from),
        _ => None,
    }
}

/// Fetch full titles for activity event notifications whose subject URL is available.
///
/// The GitHub Events API returns condensed objects (e.g. `pull_request` without
/// a `title` field), so we make a follow-up request to the subject URL and
/// append the title to the notification's subject line.
fn enrich_event_titles(client: &GitHubClient, notifications: &mut [Notification]) {
    for notif in notifications.iter_mut() {
        if notif.subject.subject_type != NotificationType::ActivityEvent {
            continue;
        }
        let url = match notif.subject.url.as_deref() {
            Some(u) => u,
            None => continue,
        };
        // Only enrich when the title is missing (no ": " suffix yet).
        if notif.subject.title.contains(": ") {
            continue;
        }
        if let Ok(value) = client.get_json_by_url(url) {
            if let Some(title) = value.get("title").and_then(|v| v.as_str()) {
                notif.subject.title = format!("{}: {}", notif.subject.title, title);
            }
        }
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
