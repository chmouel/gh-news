use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::{Notification, NotificationType, Owner, Repository, Subject};
use chrono::{DateTime, Utc};

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
    let mut all_notifications = Vec::new();

    for repo_full in repos {
        let (owner, repo_name) = match repo_full.split_once('/') {
            Some(parts) => parts,
            None => continue,
        };

        let runs = match fetch_runs(client, owner, repo_name, failed_only) {
            Ok(r) => r,
            Err(_) => continue, // Skip repos where we lack permissions
        };

        for run in runs {
            if let Some(notification) = run_to_notification(&run, repo_full, owner, repo_name) {
                all_notifications.push(notification);
            }
        }
    }

    // Sort by updated_at descending (newest first)
    all_notifications.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(all_notifications)
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
    })
}
