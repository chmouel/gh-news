use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType, Owner, Repository, Subject};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::thread;

const MAX_EVENT_WORKERS: usize = 4;

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
    if repos.is_empty() {
        return Ok(Vec::new());
    }

    if repos.len() == 1 {
        return fetch_watch_repo_events_serial(client, repos, event_types);
    }

    let worker_count = repos.len().min(MAX_EVENT_WORKERS);
    let work_queue = Arc::new(Mutex::new(VecDeque::from(repos.to_vec())));
    let event_types = Arc::new(event_types.to_vec());
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let client = client.clone();
        let work_queue = Arc::clone(&work_queue);
        let event_types = Arc::clone(&event_types);
        workers.push(thread::spawn(move || {
            let mut notifications = Vec::new();
            loop {
                let Some(repo_full) = work_queue.lock().pop_front() else {
                    break;
                };

                append_watch_repo_events(&client, &repo_full, &event_types, &mut notifications);
            }
            notifications
        }));
    }

    let mut all_notifications = Vec::new();
    for worker in workers {
        if let Ok(mut notifications) = worker.join() {
            all_notifications.append(&mut notifications);
        }
    }

    all_notifications.sort_by_key(|notification| Reverse(notification.updated_at));

    enrich_event_titles(client, &mut all_notifications);

    Ok(all_notifications)
}

fn fetch_watch_repo_events_serial(
    client: &GitHubClient,
    repos: &[String],
    event_types: &[String],
) -> Result<Vec<Notification>> {
    let mut all_notifications = Vec::new();

    for repo_full in repos {
        append_watch_repo_events(client, repo_full, event_types, &mut all_notifications);
    }

    all_notifications.sort_by_key(|notification| Reverse(notification.updated_at));

    enrich_event_titles(client, &mut all_notifications);

    Ok(all_notifications)
}

fn append_watch_repo_events(
    client: &GitHubClient,
    repo_full: &str,
    event_types: &[String],
    all_notifications: &mut Vec<Notification>,
) {
    let (owner, repo_name) = match repo_full.split_once('/') {
        Some(parts) => parts,
        None => return,
    };

    let events = match client.get_repo_events(owner, repo_name, 30) {
        Ok(e) => e,
        Err(_) => return,
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

fn json_str<'a>(value: Option<&'a serde_json::Value>, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value?, |v, key| v.get(key))?
        .as_str()
        .filter(|s| !s.is_empty())
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default().trim()
}

/// Build the one-line summary.  The repository is shown separately by every
/// view (group header, tooltip column), so it is only repeated for events
/// that have nothing else to name.
fn format_event_title(
    event_type: &str,
    actor: &str,
    repo: &str,
    event: &serde_json::Value,
) -> String {
    let payload = event.get("payload");
    match event_type {
        "WatchEvent" => format!("{actor} starred {repo}"),
        "ForkEvent" => format!("{actor} forked {repo}"),
        "CreateEvent" | "DeleteEvent" => {
            let verb = if event_type == "CreateEvent" {
                "created"
            } else {
                "deleted"
            };
            let ref_type = json_str(payload, &["ref_type"]).unwrap_or("repository");
            match json_str(payload, &["ref"]) {
                Some(name) => format!("{actor} {verb} {ref_type} {name}"),
                None => format!("{actor} {verb} {ref_type} {repo}"),
            }
        }
        "PushEvent" => {
            let count = payload
                .and_then(|p| p.get("size"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let plural = if count == 1 { "" } else { "s" };
            let branch = json_str(payload, &["ref"])
                .map(|r| r.strip_prefix("refs/heads/").unwrap_or(r))
                .map(|b| format!(" to {b}"))
                .unwrap_or_default();
            let headline = payload
                .and_then(|p| p.get("commits"))
                .and_then(|c| c.as_array())
                .and_then(|c| c.last())
                .and_then(|c| json_str(Some(c), &["message"]))
                .map(|m| format!(": {}", first_line(m)))
                .unwrap_or_default();
            format!("{actor} pushed {count} commit{plural}{branch}{headline}")
        }
        "IssuesEvent" | "PullRequestEvent" => {
            let (noun, key) = if event_type == "IssuesEvent" {
                ("issue", "issue")
            } else {
                ("PR", "pull_request")
            };
            let item = payload.and_then(|p| p.get(key));
            let phrase = action_phrase(payload, item);
            let number = item
                .and_then(|i| i.get("number"))
                .and_then(|v| v.as_u64())
                .map(|n| format!(" #{n}"))
                .unwrap_or_default();
            let title = json_str(item, &["title"])
                .map(|t| format!(": {t}"))
                .unwrap_or_default();
            format!("{actor} {phrase} {noun}{number}{title}")
        }
        "ReleaseEvent" => {
            let tag = json_str(payload, &["release", "tag_name"]).unwrap_or("a release");
            format!("{actor} released {tag}")
        }
        "IssueCommentEvent" => {
            let issue = payload.and_then(|p| p.get("issue"));
            let noun = if issue.and_then(|i| i.get("pull_request")).is_some() {
                "PR"
            } else {
                "issue"
            };
            let verb = match json_str(payload, &["action"]) {
                Some("edited") => "edited a comment on",
                Some("deleted") => "deleted a comment on",
                _ => "commented on",
            };
            let number = issue
                .and_then(|i| i.get("number"))
                .and_then(|v| v.as_u64())
                .map(|n| format!(" #{n}"))
                .unwrap_or_default();
            let title = json_str(issue, &["title"])
                .map(|t| format!(": {t}"))
                .unwrap_or_default();
            format!("{actor} {verb} {noun}{number}{title}")
        }
        "MemberEvent" => {
            let action = json_str(payload, &["action"]).unwrap_or("added");
            let member = json_str(payload, &["member", "login"]).unwrap_or("someone");
            format!("{actor} {action} {member} to {repo}")
        }
        "PublicEvent" => format!("{actor} made {repo} public"),
        "GollumEvent" => format!("{actor} updated wiki pages in {repo}"),
        _ => format!("{event_type}: {actor} in {repo}"),
    }
}

/// Turn a GitHub event action into a phrase that reads before the noun
/// ("added label \"bug\" to PR #1", "merged PR #1").
fn action_phrase(payload: Option<&serde_json::Value>, item: Option<&serde_json::Value>) -> String {
    let action = json_str(payload, &["action"]).unwrap_or("updated");
    let label = json_str(payload, &["label", "name"]);
    let assignee = json_str(payload, &["assignee", "login"]);
    let reviewer = json_str(payload, &["requested_reviewer", "login"]);
    let merged = item
        .and_then(|i| i.get("merged"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    match (action, label, assignee, reviewer) {
        ("labeled", Some(l), ..) => format!("added label \"{l}\" to"),
        ("unlabeled", Some(l), ..) => format!("removed label \"{l}\" from"),
        ("assigned", _, Some(a), _) => format!("assigned @{a} to"),
        ("unassigned", _, Some(a), _) => format!("unassigned @{a} from"),
        ("review_requested", _, _, Some(r)) => format!("requested review from @{r} on"),
        ("closed", ..) if merged => "merged".to_string(),
        ("ready_for_review", ..) => "marked ready for review".to_string(),
        ("synchronize", ..) => "pushed to".to_string(),
        (other, ..) => other.replace('_', " "),
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
    let urls: HashSet<String> = notifications
        .iter()
        .filter(|notif| {
            notif.subject.subject_type == NotificationType::ActivityEvent
                && !notif.subject.title.contains(": ")
        })
        .filter_map(|notif| notif.subject.url.clone())
        .collect();
    let worker_count = urls.len().min(MAX_EVENT_WORKERS);
    let queue = Mutex::new(VecDeque::from_iter(urls));
    let titles = thread::scope(|scope| {
        let workers: Vec<_> = (0..worker_count)
            .map(|_| {
                scope.spawn(|| {
                    let mut titles = HashMap::new();
                    loop {
                        let Some(url) = queue.lock().pop_front() else {
                            break;
                        };
                        if let Ok(value) = client.get_json_by_url(&url) {
                            if let Some(title) = value.get("title").and_then(|v| v.as_str()) {
                                titles.insert(url, title.to_owned());
                            }
                        }
                    }
                    titles
                })
            })
            .collect();
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .flatten()
            .collect::<HashMap<_, _>>()
    });
    for notif in notifications {
        if notif.subject.subject_type == NotificationType::ActivityEvent
            && !notif.subject.title.contains(": ")
        {
            if let Some(title) = notif.subject.url.as_ref().and_then(|url| titles.get(url)) {
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

#[cfg(test)]
mod enrichment_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn title_lookups_are_deduplicated_bounded_and_concurrent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let server_peak = peak.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut workers = Vec::new();
            while workers.len() < 8 && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let active = active.clone();
                        let peak = server_peak.clone();
                        workers.push(thread::spawn(move || {
                            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                            let mut buf = [0; 4096];
                            let size = stream.read(&mut buf).unwrap();
                            let request = String::from_utf8_lossy(&buf[..size]);
                            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(count, Ordering::SeqCst);
                            thread::sleep(Duration::from_millis(100));
                            let (status, body) = if request.contains(" /0 ") {
                                ("500 Error", "{}")
                            } else if request.contains(" /1 ") {
                                ("200 OK", "{}")
                            } else {
                                ("200 OK", r#"{"title":"Full title"}"#)
                            };
                            active.fetch_sub(1, Ordering::SeqCst);
                            write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("{error}"),
                }
            }
            let count = workers.len();
            for worker in workers {
                worker.join().unwrap();
            }
            count
        });
        let mut notifications: Vec<_> = (0..16)
            .map(|i| {
                let mut notification = event_to_notification(
                    &serde_json::json!({"id": i.to_string()}),
                    "IssuesEvent",
                    "event-",
                )
                .unwrap();
                notification.subject.title = format!("Event {i}");
                notification.subject.url = Some(format!("{base}/{}", i % 8));
                notification
            })
            .collect();
        let mut existing = notifications[2].clone();
        existing.subject.title = "Existing: title".into();
        notifications.push(existing);
        enrich_event_titles(&GitHubClient::new_test_with_base(base), &mut notifications);
        assert_eq!(server.join().unwrap(), 8);
        assert!((2..=MAX_EVENT_WORKERS).contains(&peak.load(Ordering::SeqCst)));
        for (i, notification) in notifications[..16].iter().enumerate() {
            let expected = if i % 8 < 2 {
                format!("Event {i}")
            } else {
                format!("Event {i}: Full title")
            };
            assert_eq!(notification.subject.title, expected);
        }
        assert_eq!(notifications[16].subject.title, "Existing: title");
    }
}

#[cfg(test)]
mod title_tests {
    use super::*;
    use serde_json::json;

    fn title(event_type: &str, payload: serde_json::Value) -> String {
        format_event_title(
            event_type,
            "alice",
            "owner/repo",
            &json!({ "payload": payload }),
        )
    }

    #[test]
    fn labels_and_assignees_read_naturally_without_the_repository() {
        let pr = json!({ "number": 7, "title": "Fix it" });
        assert_eq!(
            title(
                "PullRequestEvent",
                json!({ "action": "labeled", "label": { "name": "bug" }, "pull_request": pr })
            ),
            "alice added label \"bug\" to PR #7: Fix it"
        );
        assert_eq!(
            title(
                "IssuesEvent",
                json!({ "action": "assigned", "assignee": { "login": "bob" }, "issue": { "number": 3 } })
            ),
            "alice assigned @bob to issue #3"
        );
    }

    #[test]
    fn merged_pull_requests_say_merged() {
        let payload = json!({
            "action": "closed",
            "pull_request": { "number": 9, "merged": true }
        });
        assert_eq!(title("PullRequestEvent", payload), "alice merged PR #9");
    }

    #[test]
    fn pushes_name_the_branch_and_headline() {
        let payload = json!({
            "ref": "refs/heads/main",
            "size": 2,
            "commits": [{ "message": "first" }, { "message": "second\n\nbody" }]
        });
        assert_eq!(
            title("PushEvent", payload),
            "alice pushed 2 commits to main: second"
        );
        assert_eq!(
            title("PushEvent", json!({ "ref": "refs/heads/dev", "size": 1 })),
            "alice pushed 1 commit to dev"
        );
    }

    #[test]
    fn comments_distinguish_pull_requests_from_issues() {
        let on_pr = json!({
            "action": "created",
            "issue": { "number": 4, "title": "T", "pull_request": {} }
        });
        assert_eq!(
            title("IssueCommentEvent", on_pr),
            "alice commented on PR #4: T"
        );
        let on_issue = json!({ "issue": { "number": 5, "title": "T" } });
        assert_eq!(
            title("IssueCommentEvent", on_issue),
            "alice commented on issue #5: T"
        );
    }
}
