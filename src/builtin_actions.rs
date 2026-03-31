use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::Notification;
use crate::state_file::AppStateFile;
use chrono::{Duration, Utc};

/// Built-in actions that are always available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinAction {
    MuteThread,
    MuteRepository,
    SnoozeUntilLater,
    SnoozeUntilTomorrow,
    SnoozeUntilNextWeek,
}

impl BuiltinAction {
    /// Get all available built-in actions
    pub fn all() -> Vec<Self> {
        vec![
            Self::MuteThread,
            Self::MuteRepository,
            Self::SnoozeUntilLater,
            Self::SnoozeUntilTomorrow,
            Self::SnoozeUntilNextWeek,
        ]
    }

    /// Get the display name for this action
    pub fn name(&self) -> &'static str {
        match self {
            Self::MuteThread => "Mute Thread",
            Self::MuteRepository => "Mute Repository",
            Self::SnoozeUntilLater => "Snooze (4 hours)",
            Self::SnoozeUntilTomorrow => "Snooze (tomorrow 09:00)",
            Self::SnoozeUntilNextWeek => "Snooze (next week)",
        }
    }

    /// Execute this built-in action on a single notification
    pub fn execute(&self, notification: &Notification, client: &GitHubClient) -> Result<String> {
        match self {
            Self::MuteThread => {
                client.mute_thread(&notification.id)?;
                Ok(format!("Muted thread: {}", notification.title()))
            }
            Self::MuteRepository => {
                let repo_full = notification.repo_full_name();
                let (owner, repo) = repo_full.split_once('/').ok_or_else(|| {
                    crate::error::Error::Config("Invalid repository name".to_string())
                })?;
                client.mute_repository(owner, repo)?;
                Ok(format!("Muted repository: {}", repo_full))
            }
            Self::SnoozeUntilLater => {
                let wake_time = Utc::now() + Duration::hours(4);
                AppStateFile::snooze_notification(notification.id.clone(), wake_time, None)?;
                Ok(format!(
                    "Snoozed until later (4 hours): {}",
                    notification.title()
                ))
            }
            Self::SnoozeUntilTomorrow => {
                let wake_time = (Utc::now() + Duration::days(1))
                    .date_naive()
                    .and_hms_opt(9, 0, 0)
                    .expect("9:00:00 is a valid time")
                    .and_utc();
                AppStateFile::snooze_notification(notification.id.clone(), wake_time, None)?;
                Ok(format!(
                    "Snoozed until tomorrow 09:00: {}",
                    notification.title()
                ))
            }
            Self::SnoozeUntilNextWeek => {
                let wake_time = (Utc::now() + Duration::weeks(1))
                    .date_naive()
                    .and_hms_opt(9, 0, 0)
                    .expect("9:00:00 is a valid time")
                    .and_utc();
                AppStateFile::snooze_notification(notification.id.clone(), wake_time, None)?;
                Ok(format!("Snoozed until next week: {}", notification.title()))
            }
        }
    }

    /// Execute this built-in action on multiple notifications
    pub fn execute_batch(
        &self,
        notifications: &[Notification],
        client: &GitHubClient,
    ) -> Result<String> {
        let mut success_count = 0;
        let mut last_error = None;

        for notification in notifications {
            match self.execute(notification, client) {
                Ok(_) => success_count += 1,
                Err(e) => last_error = Some(e),
            }
        }

        if let Some(e) = last_error {
            if success_count == 0 {
                return Err(e);
            }
        }

        Ok(format!(
            "{}: {} of {} notifications",
            self.name(),
            success_count,
            notifications.len()
        ))
    }
}

/// Combined action that can be either built-in or custom
#[derive(Debug, Clone)]
pub enum CombinedAction {
    Builtin(BuiltinAction),
    Custom(crate::config::Action),
}

impl CombinedAction {
    /// Get the display name
    pub fn name(&self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::Builtin(action) => std::borrow::Cow::Borrowed(action.name()),
            Self::Custom(action) => std::borrow::Cow::Owned(action.name.clone()),
        }
    }
}

/// Get all actions (built-in + custom)
pub fn get_all_actions(custom_actions: &[crate::config::Action]) -> Vec<CombinedAction> {
    let mut actions = Vec::new();

    // Add built-in actions first
    for builtin in BuiltinAction::all() {
        actions.push(CombinedAction::Builtin(builtin));
    }

    // Add custom actions
    for custom in custom_actions {
        actions.push(CombinedAction::Custom(custom.clone()));
    }

    actions
}
