use crate::config::Config;
use crate::error::{Error, Result};
use crate::models::Notification;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct NotificationCache {
    version: u32,
    timestamp: DateTime<Utc>,
    fetch_options_hash: String,
    notifications: Vec<Notification>,
}

/// Build a deterministic key from the fetch parameters so the cache is
/// invalidated when the user changes flags (e.g. `--all`).
pub fn compute_options_hash(
    show_all: bool,
    participating: bool,
    max_notifications: Option<usize>,
    config: &Config,
) -> String {
    format!(
        concat!(
            "all={},participating={},max={:?},per_page={},host={},",
            "actions={},actions_failed_only={},actions_repos={:?},",
            "events={},event_types={:?},watch_repos={:?}"
        ),
        show_all,
        participating,
        max_notifications,
        config.pagination_size,
        config.github_host,
        config.enable_actions,
        config.actions_failed_only,
        config.actions_repos,
        config.enable_events,
        config.event_types,
        config.watch_repos
    )
}

/// Return the cache file path, using a custom override or the default XDG
/// cache location.
pub fn get_cache_path(custom_path: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = custom_path {
        return Ok(PathBuf::from(p));
    }

    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;

    let dir = cache_dir.join("gh-news");
    fs::create_dir_all(&dir).map_err(Error::Io)?;

    Ok(dir.join("notifications_cache.json"))
}

/// Try to load cached notifications.  Returns `Some` when the file exists
/// and the version and options hash match.  An empty cache is treated as a
/// miss so startup performs a full refresh instead of showing an empty
/// list.  No TTL check -- the caller always triggers a background refresh
/// after displaying cached data.
pub fn load_cache(cache_path: &Path, options_hash: &str) -> Option<Vec<Notification>> {
    let content = fs::read_to_string(cache_path).ok()?;
    let cache: NotificationCache = serde_json::from_str(&content).ok()?;

    if cache.version != CACHE_VERSION {
        return None;
    }
    if cache.fetch_options_hash != options_hash {
        return None;
    }
    if cache.notifications.is_empty() {
        return None;
    }

    Some(cache.notifications)
}

/// Persist notifications to the cache file.  Uses an atomic write (temp file
/// + rename) so concurrent readers never see a partial file.
pub fn save_cache(
    cache_path: &Path,
    notifications: &[Notification],
    options_hash: &str,
) -> Result<()> {
    let cache = NotificationCache {
        version: CACHE_VERSION,
        timestamp: Utc::now(),
        fetch_options_hash: options_hash.to_string(),
        notifications: notifications.to_vec(),
    };

    let json = serde_json::to_string(&cache)
        .map_err(|e| Error::Config(format!("Failed to serialise cache: {}", e)))?;

    let tmp_path = cache_path.with_extension("json.tmp");

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    fs::write(&tmp_path, json).map_err(Error::Io)?;
    fs::rename(&tmp_path, cache_path).map_err(Error::Io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("gh-news-cache-test-{}-{}", std::process::id(), id));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_notification() -> Notification {
        serde_json::from_str(
            r#"{
                "id": "1",
                "unread": true,
                "reason": "mention",
                "repository": {
                    "id": 1,
                    "name": "repo",
                    "full_name": "owner/repo",
                    "owner": { "login": "owner", "id": 1, "type": "User" },
                    "private": false
                },
                "subject": { "title": "Test", "type": "Issue", "url": null }
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn round_trip() {
        let dir = test_dir();
        let path = dir.join("cache.json");
        let hash = compute_options_hash(false, false, None, &Config::default());
        let notifications = vec![sample_notification()];

        save_cache(&path, &notifications, &hash).unwrap();

        let loaded = load_cache(&path, &hash).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mismatched_hash_returns_none() {
        let dir = test_dir();
        let path = dir.join("cache.json");

        save_cache(&path, &[sample_notification()], "hash_a").unwrap();
        assert!(load_cache(&path, "hash_b").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_cache_is_treated_as_miss() {
        let dir = test_dir();
        let path = dir.join("cache.json");

        save_cache(&path, &[], "hash").unwrap();
        assert!(load_cache(&path, "hash").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_returns_none() {
        let dir = test_dir();
        let path = dir.join("nonexistent.json");
        assert!(load_cache(&path, "any").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn options_hash_changes_when_extra_sources_change() {
        let base = Config::default();
        let mut with_actions = Config {
            enable_actions: true,
            ..Config::default()
        };
        with_actions.actions_repos = vec!["owner/repo".to_string()];

        assert_ne!(
            compute_options_hash(false, false, None, &base),
            compute_options_hash(false, false, None, &with_actions)
        );
    }
}
