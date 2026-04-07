use crate::api::GitHubClient;
use crate::config::Action;
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
    Custom(Action),
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

/// Map an action index to a keyboard shortcut character.
/// Indices 0-8 map to '1'-'9', indices 9+ map to 'a'-'z' (skipping 'j' and
/// 'k' which are reserved for navigation).
pub fn shortcut_for_index(index: usize) -> Option<char> {
    match index {
        0..=8 => Some((b'1' + index as u8) as char),
        9.. => {
            // Skip 'j' (9th letter) and 'k' (10th letter)
            let offset = index - 9;
            let c = match offset {
                0..=8 => b'a' + offset as u8, // a-i
                9.. => {
                    let shifted = offset + 2; // skip j, k
                    if shifted > 25 {
                        return None;
                    }
                    b'a' + shifted as u8 // l-z
                }
            };
            Some(c as char)
        }
    }
}

/// Find the action index for a given shortcut character.
/// Accepts both upper and lowercase letters; 'j' and 'k' are excluded
/// because they are used for navigation in the action menu.
pub fn index_for_shortcut(c: char) -> Option<usize> {
    let c = c.to_ascii_lowercase();
    match c {
        '1'..='9' => Some((c as u8 - b'1') as usize),
        'a'..='i' => Some((c as u8 - b'a') as usize + 9),
        'l'..='z' => Some((c as u8 - b'a' - 2) as usize + 9), // subtract 2 for skipped j,k
        _ => None,
    }
}

/// Get all actions (built-in + custom)
pub fn get_all_actions(custom_actions: &[Action]) -> Vec<CombinedAction> {
    #[derive(Debug)]
    struct OrderedAction {
        sort_bucket: u8,
        priority: Option<i32>,
        original_index: usize,
        action: CombinedAction,
    }

    let mut actions = Vec::new();

    for (original_index, builtin) in BuiltinAction::all().into_iter().enumerate() {
        actions.push(OrderedAction {
            sort_bucket: 1,
            priority: None,
            original_index,
            action: CombinedAction::Builtin(builtin),
        });
    }

    let builtin_count = actions.len();
    for (index, custom) in custom_actions.iter().enumerate() {
        let sort_bucket = if custom.priority.is_some() { 0 } else { 2 };
        actions.push(OrderedAction {
            sort_bucket,
            priority: custom.priority,
            original_index: builtin_count + index,
            action: CombinedAction::Custom(custom.clone()),
        });
    }

    actions.sort_by_key(|entry| (entry.sort_bucket, entry.priority, entry.original_index));
    actions.into_iter().map(|entry| entry.action).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom_action(name: &str, priority: Option<i32>) -> Action {
        Action {
            name: name.to_string(),
            command: "true".to_string(),
            priority,
            interactive: false,
            show_output: false,
        }
    }

    fn action_names(actions: Vec<CombinedAction>) -> Vec<String> {
        actions
            .into_iter()
            .map(|action| action.name().into_owned())
            .collect()
    }

    #[test]
    fn test_get_all_actions_puts_prioritised_custom_actions_before_builtins() {
        let actions = get_all_actions(&[custom_action("Top action", Some(1))]);

        assert_eq!(action_names(actions)[0], "Top action");
    }

    #[test]
    fn test_get_all_actions_sorts_custom_actions_by_priority() {
        let actions = get_all_actions(&[
            custom_action("Priority 20", Some(20)),
            custom_action("Priority 1", Some(1)),
            custom_action("Priority 5", Some(5)),
        ]);

        let names = action_names(actions);
        assert_eq!(names[0], "Priority 1");
        assert_eq!(names[1], "Priority 5");
        assert_eq!(names[2], "Priority 20");
    }

    #[test]
    fn test_get_all_actions_preserves_order_for_matching_priorities() {
        let actions = get_all_actions(&[
            custom_action("First", Some(1)),
            custom_action("Second", Some(1)),
        ]);

        let names = action_names(actions);
        assert_eq!(names[0], "First");
        assert_eq!(names[1], "Second");
    }

    #[test]
    fn test_get_all_actions_keeps_unprioritised_custom_actions_after_builtins() {
        let actions = get_all_actions(&[custom_action("Default custom", None)]);

        let names = action_names(actions);
        let builtins = BuiltinAction::all();
        assert_eq!(names[builtins.len() - 1], builtins.last().unwrap().name());
        assert_eq!(names[builtins.len()], "Default custom");
    }
}
