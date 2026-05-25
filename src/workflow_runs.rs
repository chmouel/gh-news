use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType, Owner, Repository, Subject};
use chrono::{DateTime, Utc};
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const MAX_WORKFLOW_RUN_WORKERS: usize = 4;

/// Fetch workflow run notifications from GitHub Actions for the given repos.
///
/// When `failed_only` is true, only failed/cancelled runs are returned.
/// When `repos` is empty, no runs are fetched (caller should derive repos
/// from existing notifications).
pub fn fetch_workflow_run_notifications(
    client: &GitHubClient,
    repos: &[String],
    failed_only: bool,
) -> Result<Vec<Notification>> {
    if repos.is_empty() {
        return Ok(Vec::new());
    }

    if repos.len() == 1 {
        return fetch_workflow_run_notifications_serial(client, repos, failed_only);
    }

    let worker_count = repos.len().min(MAX_WORKFLOW_RUN_WORKERS);
    let work_queue = Arc::new(Mutex::new(VecDeque::from(repos.to_vec())));
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let client = client.clone();
        let work_queue = Arc::clone(&work_queue);
        workers.push(thread::spawn(move || {
            let mut notifications = Vec::new();
            loop {
                let Some(repo_full) = work_queue.lock().unwrap().pop_front() else {
                    break;
                };

                append_repo_workflow_runs(&client, &repo_full, failed_only, &mut notifications);
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

    // Sort by updated_at descending (newest first)
    all_notifications.sort_by_key(|notification| Reverse(notification.updated_at));

    Ok(all_notifications)
}

fn fetch_workflow_run_notifications_serial(
    client: &GitHubClient,
    repos: &[String],
    failed_only: bool,
) -> Result<Vec<Notification>> {
    let mut all_notifications = Vec::new();

    for repo_full in repos {
        append_repo_workflow_runs(client, repo_full, failed_only, &mut all_notifications);
    }

    // Sort by updated_at descending (newest first)
    all_notifications.sort_by_key(|notification| Reverse(notification.updated_at));

    Ok(all_notifications)
}

fn append_repo_workflow_runs(
    client: &GitHubClient,
    repo_full: &str,
    failed_only: bool,
    all_notifications: &mut Vec<Notification>,
) {
    let (owner, repo_name) = match repo_full.split_once('/') {
        Some(parts) => parts,
        None => return,
    };

    let runs = match fetch_runs(client, owner, repo_name, failed_only) {
        Ok(r) => r,
        Err(_) => return, // Skip repos where we lack permissions
    };

    for run in runs {
        if let Some(notification) = run_to_notification(&run, repo_full, owner, repo_name) {
            all_notifications.push(notification);
        }
    }
}

fn fetch_runs(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    failed_only: bool,
) -> Result<Vec<serde_json::Value>> {
    if failed_only {
        // GitHub's API accepts only one status per request, so fetch both
        // failed and cancelled runs separately and merge them.
        let mut runs = Vec::new();
        for status in &["failure", "cancelled"] {
            if let Ok(data) = client.get_workflow_runs(owner, repo, Some(status), 10) {
                if let Some(arr) = data.get("workflow_runs").and_then(|v| v.as_array()) {
                    runs.extend(arr.iter().cloned());
                }
            }
        }
        Ok(runs)
    } else {
        let data = client.get_workflow_runs(owner, repo, None, 10)?;
        Ok(data
            .get("workflow_runs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }
}

fn run_to_notification(
    run: &serde_json::Value,
    repo_full: &str,
    owner: &str,
    repo_name: &str,
) -> Option<Notification> {
    let run_id = run.get("id")?.as_u64()?;
    let workflow_name = run
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Workflow");
    let run_number = run.get("run_number").and_then(|v| v.as_u64()).unwrap_or(0);
    let conclusion = run
        .get("conclusion")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let updated_at = run
        .get("updated_at")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());
    let html_url = run.get("html_url").and_then(|v| v.as_str());

    let title = format!("{} #{} ({})", workflow_name, run_number, conclusion);

    Some(Notification {
        id: format!("actions-{}", run_id),
        unread: true,
        last_read_at: None,
        updated_at,
        reason: "ci_activity".to_string(),
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
            subject_type: NotificationType::WorkflowRun,
            url: html_url.map(String::from),
            latest_comment_url: None,
        },
        latest_comment_url: None,
        author: None,
        context: None,
        event_body: None,
    })
}
