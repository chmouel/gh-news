use crate::config::View;

/// Returns the built-in named views, prepended before any user-defined views.
/// These cover the most common notification triage scenarios.
pub fn builtin_views() -> Vec<View> {
    vec![
        // Show only notifications where you're actively involved.
        // Excludes passive subscriptions and bot CI noise.
        View {
            name: "Participating".to_string(),
            filter: None,
            exclude_types: None,
            exclude_reasons: Some(vec!["subscribed".to_string(), "ci_activity".to_string()]),
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Direct @mentions and team mentions.
        // Matches reason ending in "mention" (covers both "mention" and "team_mention").
        View {
            name: "Mentions".to_string(),
            filter: Some("mention$".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // PRs where your review has been requested.
        View {
            name: "Review Requests".to_string(),
            filter: Some("review_requested$".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Issues and PRs assigned to you.
        View {
            name: "Assigned".to_string(),
            filter: Some("assign$".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Notifications on things you opened or created.
        View {
            name: "My Activity".to_string(),
            filter: Some("author$".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Security alerts only.
        View {
            name: "Security".to_string(),
            filter: Some("security".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Dependabot version bumps and security alerts.
        // Matches "Bump X from Y to Z" PR titles and any notification where
        // "dependabot" appears in the repo name or title.
        View {
            name: "Dependabot".to_string(),
            filter: Some("(?i)(dependabot|\\bBump\\b)".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
        // Activity from automated bots (Copilot, CodeRabbit, Dependabot, Renovate, etc.).
        // GitHub bot accounts always end with "[bot]" in their login name.
        // Matched against the author field populated by background enrichment —
        // notifications may appear here only after a short delay on first run.
        View {
            name: "Bots".to_string(),
            filter: Some("(?i)\\[bot\\]".to_string()),
            exclude_types: None,
            exclude_reasons: None,
            exclude_repos: None,
            exclude_subjects: None,
        },
    ]
}
