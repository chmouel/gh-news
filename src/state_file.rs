use crate::error::{Error, Result};
use crate::models::Notification;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global state file path, set once at startup
static STATE_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Initialise the state file path. Call this once at startup.
/// If `custom_path` is provided, use it; otherwise use the default XDG cache path.
pub fn init_state_path(custom_path: Option<PathBuf>) -> Result<()> {
    let path = match custom_path {
        Some(p) => {
            // Ensure parent directory exists for custom paths
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).map_err(Error::Io)?;
            }
            p
        }
        None => get_default_state_path()?,
    };

    STATE_FILE_PATH
        .set(path)
        .map_err(|_| Error::Config("State path already initialised".to_string()))?;
    Ok(())
}

/// Get the default XDG cache state path
fn get_default_state_path() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;

    let state_dir = cache_dir.join("gh-news");
    fs::create_dir_all(&state_dir).map_err(Error::Io)?;

    Ok(state_dir.join("state.toml"))
}

/// Get the configured state file path
fn get_state_path() -> Result<PathBuf> {
    STATE_FILE_PATH
        .get()
        .cloned()
        .ok_or_else(|| Error::Config("State path not initialised".to_string()))
}

/// Snooze information for a notification thread
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnoozeEntry {
    /// When the notification should become visible again
    pub wake_time: DateTime<Utc>,
    /// Optional note about why it was snoozed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    #[serde(default = "default_auto_mark_read")]
    pub auto_mark_read: bool,
    #[serde(default)]
    pub auto_archive: bool,
    #[serde(default)]
    pub pinned_notifications: Vec<Notification>,
    /// Map of notification thread ID to snooze information
    #[serde(default)]
    pub snoozed_notifications: HashMap<String, SnoozeEntry>,
    #[serde(default)]
    pub dismissed_synthetic_ids: Vec<String>,
}

fn default_auto_mark_read() -> bool {
    true
}

impl AppStateFile {
    pub fn new(auto_mark_read: bool) -> Self {
        Self {
            auto_mark_read,
            auto_archive: false,
            pinned_notifications: Vec::new(),
            snoozed_notifications: HashMap::new(),
            dismissed_synthetic_ids: Vec::new(),
        }
    }

    fn load_full() -> Result<AppStateFile> {
        let path = get_state_path()?;

        if !path.exists() {
            return Err(Error::Config("State file does not exist".to_string()));
        }

        let content = fs::read_to_string(&path).map_err(Error::Io)?;

        let state: AppStateFile = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse state: {}", e)))?;

        Ok(state)
    }

    fn save_full(&self) -> Result<()> {
        let path = get_state_path()?;

        let toml_content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize state: {}", e)))?;

        fs::write(&path, toml_content).map_err(Error::Io)?;

        Ok(())
    }

    fn load_or_default(auto_mark_read: bool) -> AppStateFile {
        Self::load_full().unwrap_or_else(|_| Self::new(auto_mark_read))
    }

    fn update_with<F>(update: F) -> Result<()>
    where
        F: FnOnce(&mut AppStateFile),
    {
        let mut state = Self::load_or_default(default_auto_mark_read());
        update(&mut state);
        state.save_full()
    }

    pub fn save_auto_mark_read(auto_mark_read: bool) -> Result<()> {
        Self::update_with(|state| {
            state.auto_mark_read = auto_mark_read;
        })
    }

    pub fn load_auto_mark_read() -> Result<bool> {
        let state = Self::load_full()?;
        Ok(state.auto_mark_read)
    }

    #[allow(dead_code)]
    pub fn save_auto_archive(auto_archive: bool) -> Result<()> {
        Self::update_with(|state| {
            state.auto_archive = auto_archive;
        })
    }

    pub fn load_auto_archive() -> Result<bool> {
        let state = Self::load_full()?;
        Ok(state.auto_archive)
    }

    pub fn save_pinned_notifications(pinned: &[Notification]) -> Result<()> {
        Self::update_with(|state| {
            state.pinned_notifications = pinned.to_vec();
        })
    }

    pub fn load_pinned_notifications() -> Result<Vec<Notification>> {
        let state = Self::load_full()?;
        Ok(state.pinned_notifications)
    }

    /// Load snoozed notifications, filtering out expired entries
    pub fn load_snoozed_notifications() -> Result<HashMap<String, SnoozeEntry>> {
        let state = Self::load_full()?;
        let now = Utc::now();

        // Filter out expired snoozes
        let active_snoozes = state
            .snoozed_notifications
            .into_iter()
            .filter(|(_, entry)| entry.wake_time > now)
            .collect();

        Ok(active_snoozes)
    }

    /// Add or update a snooze entry for a notification
    pub fn snooze_notification(
        thread_id: String,
        wake_time: DateTime<Utc>,
        note: Option<String>,
    ) -> Result<()> {
        Self::update_with(|state| {
            state
                .snoozed_notifications
                .insert(thread_id, SnoozeEntry { wake_time, note });
        })
    }

    /// Record a synthetic notification ID as dismissed so it is filtered out
    /// on future fetches. The list is capped at 200 entries.
    pub fn dismiss_synthetic_id(id: &str) -> Result<()> {
        Self::update_with(|state| {
            if !state.dismissed_synthetic_ids.iter().any(|d| d == id) {
                state.dismissed_synthetic_ids.push(id.to_string());
            }
            Self::trim_dismissed(&mut state.dismissed_synthetic_ids);
        })
    }

    /// Batch-dismiss multiple synthetic notification IDs.
    pub fn dismiss_synthetic_ids(ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        Self::update_with(|state| {
            for id in ids {
                if !state.dismissed_synthetic_ids.iter().any(|d| d == id) {
                    state.dismissed_synthetic_ids.push(id.clone());
                }
            }
            Self::trim_dismissed(&mut state.dismissed_synthetic_ids);
        })
    }

    /// Load the set of dismissed synthetic IDs for filtering.
    pub fn load_dismissed_synthetic_ids() -> Result<HashSet<String>> {
        let state = Self::load_full()?;
        Ok(state.dismissed_synthetic_ids.into_iter().collect())
    }

    /// Keep only the most recent 200 entries, dropping the oldest.
    fn trim_dismissed(ids: &mut Vec<String>) {
        const MAX_DISMISSED: usize = 200;
        if ids.len() > MAX_DISMISSED {
            let excess = ids.len() - MAX_DISMISSED;
            ids.drain(..excess);
        }
    }
}

/// Path of the author cache file (sibling of the state file).
fn get_author_cache_path() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;
    let dir = cache_dir.join("gh-news");
    fs::create_dir_all(&dir).map_err(Error::Io)?;
    Ok(dir.join("authors.json"))
}

/// Persist a notification-id → author-login mapping to disk.
pub fn save_author_cache(authors: &HashMap<String, String>) -> Result<()> {
    let path = get_author_cache_path()?;
    let json = serde_json::to_string(authors).map_err(|e| Error::Config(e.to_string()))?;
    fs::write(&path, json).map_err(Error::Io)
}

/// Load the previously persisted author cache, returning an empty map on any error.
pub fn load_author_cache() -> HashMap<String, String> {
    get_author_cache_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Path of the context cache file (sibling of the state file).
fn get_context_cache_path() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;
    let dir = cache_dir.join("gh-news");
    fs::create_dir_all(&dir).map_err(Error::Io)?;
    Ok(dir.join("contexts.json"))
}

/// Persist a notification-id → context mapping to disk.
pub fn save_context_cache(contexts: &HashMap<String, String>) -> Result<()> {
    let path = get_context_cache_path()?;
    let json = serde_json::to_string(contexts).map_err(|e| Error::Config(e.to_string()))?;
    fs::write(&path, json).map_err(Error::Io)
}

/// Load the previously persisted context cache, returning an empty map on any error.
pub fn load_context_cache() -> HashMap<String, String> {
    get_context_cache_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_state_with_snoozes(entries: &[(&str, DateTime<Utc>)]) -> AppStateFile {
        let mut state = AppStateFile::new(true);
        for (id, wake_time) in entries {
            state.snoozed_notifications.insert(
                id.to_string(),
                SnoozeEntry {
                    wake_time: *wake_time,
                    note: None,
                },
            );
        }
        state
    }

    #[test]
    fn snooze_entry_is_stored() {
        let wake_time = Utc::now() + Duration::hours(4);
        let mut state = AppStateFile::new(true);
        state.snoozed_notifications.insert(
            "thread-1".to_string(),
            SnoozeEntry {
                wake_time,
                note: None,
            },
        );
        assert!(state.snoozed_notifications.contains_key("thread-1"));
        assert_eq!(state.snoozed_notifications["thread-1"].wake_time, wake_time);
    }

    #[test]
    fn expired_snoozes_filtered_on_load() {
        let now = Utc::now();
        let state = make_state_with_snoozes(&[
            ("active", now + Duration::hours(1)),
            ("expired", now - Duration::minutes(1)),
        ]);

        let active: HashMap<String, SnoozeEntry> = state
            .snoozed_notifications
            .into_iter()
            .filter(|(_, e)| e.wake_time > now)
            .collect();

        assert!(active.contains_key("active"));
        assert!(!active.contains_key("expired"));
    }

    #[test]
    fn snooze_note_is_optional() {
        let wake_time = Utc::now() + Duration::days(1);
        let with_note = SnoozeEntry {
            wake_time,
            note: Some("waiting for review".to_string()),
        };
        let without_note = SnoozeEntry {
            wake_time,
            note: None,
        };

        assert_eq!(with_note.note.as_deref(), Some("waiting for review"));
        assert!(without_note.note.is_none());
    }

    #[test]
    fn snooze_serialises_and_deserialises() {
        let wake_time = Utc::now() + Duration::hours(4);
        let mut state = AppStateFile::new(true);
        state.snoozed_notifications.insert(
            "t1".to_string(),
            SnoozeEntry {
                wake_time,
                note: Some("later".to_string()),
            },
        );

        let toml_str = toml::to_string_pretty(&state).unwrap();
        let restored: AppStateFile = toml::from_str(&toml_str).unwrap();

        assert!(restored.snoozed_notifications.contains_key("t1"));
        let entry = &restored.snoozed_notifications["t1"];
        // Wake time round-trips (truncate to seconds for TOML precision)
        assert_eq!(entry.wake_time.timestamp(), wake_time.timestamp());
        assert_eq!(entry.note.as_deref(), Some("later"));
    }

    #[test]
    fn state_without_snoozes_deserialises() {
        // Minimal state with no snoozed_notifications key should default to empty map
        let toml_str = "auto_mark_read = true\n";
        let state: AppStateFile = toml::from_str(toml_str).unwrap();
        assert!(state.snoozed_notifications.is_empty());
    }

    #[test]
    fn multiple_snoozes_independent() {
        let now = Utc::now();
        let state = make_state_with_snoozes(&[
            ("t1", now + Duration::hours(1)),
            ("t2", now + Duration::hours(2)),
            ("t3", now + Duration::days(7)),
        ]);
        assert_eq!(state.snoozed_notifications.len(), 3);
    }
}
