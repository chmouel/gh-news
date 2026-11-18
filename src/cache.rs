use crate::config::Config;
use crate::error::{Error, Result};
use crate::models::Notification;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct NotificationCache {
    version: u32,
    timestamp: DateTime<Utc>,
    fetch_options_hash: String,
    // Status-bar invalidation token captured before the fetch started; empty
    // for the TUI cache and for entries written before the field existed.
    #[serde(default)]
    generation: String,
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

/// Cache path used by `--json`.  It is kept separate from the TUI cache so
/// the short-lived waybar polling can never evict what the interactive mode
/// has just loaded.
pub fn get_json_cache_path() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;

    let dir = cache_dir.join("gh-news");
    fs::create_dir_all(&dir).map_err(Error::Io)?;

    Ok(dir.join("notifications_json_cache.json"))
}

/// File holding the opaque token that tells `--json` its cache is outdated.
/// The TUI replaces the token when what a status bar shows should change; the
/// bar then fetches again on its next poll.
pub fn get_json_generation_path() -> Result<PathBuf> {
    let json_cache = get_json_cache_path()?;
    Ok(json_cache.with_file_name("json_cache_generation"))
}

/// Current invalidation token; a missing file reads as the empty token.
pub fn read_generation(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|token| token.trim().to_string())
        .unwrap_or_default()
}

/// Replace the invalidation token.  Best effort: a failure only means the
/// status bar keeps its cache until the TTL expires.
pub fn bump_generation(path: &Path) {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let token = format!(
        "{:x}-{:x}-{:x}",
        nanos,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let _ = write_atomically(path, &token);
}

/// Write via a uniquely named temp file so concurrent writers never share one.
fn write_atomically(path: &Path, content: &str) -> Result<()> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    let tmp_path = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp_path, content).map_err(Error::Io)?;
    fs::rename(&tmp_path, path).map_err(Error::Io)?;
    Ok(())
}

fn read_cache(
    cache_path: &Path,
    options_hash: &str,
    allow_empty: bool,
) -> Option<NotificationCache> {
    let content = fs::read_to_string(cache_path).ok()?;
    let cache: NotificationCache = serde_json::from_str(&content).ok()?;

    if cache.version != CACHE_VERSION {
        return None;
    }
    if cache.fetch_options_hash != options_hash {
        return None;
    }
    if !allow_empty && cache.notifications.is_empty() {
        return None;
    }

    Some(cache)
}

/// Try to load cached notifications.  Returns `Some` when the file exists
/// and the version and options hash match.  An empty cache is treated as a
/// miss so startup performs a full refresh instead of showing an empty
/// list.  No TTL check -- the caller always triggers a background refresh
/// after displaying cached data.
pub fn load_cache(cache_path: &Path, options_hash: &str) -> Option<Vec<Notification>> {
    read_cache(cache_path, options_hash, false).map(|cache| cache.notifications)
}

/// Load cached notifications for `--json` only when the entry is younger than
/// `max_age` and was fetched under the current invalidation `generation`.
/// Status bars poll on a short timer, so a stale entry must trigger a fetch,
/// and a genuinely empty inbox is a valid hit rather than a reason to refetch.
pub fn load_cache_fresh(
    cache_path: &Path,
    options_hash: &str,
    max_age: chrono::Duration,
    generation: &str,
) -> Option<Vec<Notification>> {
    let cache = read_cache(cache_path, options_hash, true)?;
    if cache.generation != generation || Utc::now().signed_duration_since(cache.timestamp) > max_age
    {
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
    save_cache_with_generation(cache_path, notifications, options_hash, "")
}

/// Like [`save_cache`], recording the invalidation token that was current
/// before the fetch began, so an entry fetched across an invalidation is
/// never served as fresh.
pub fn save_cache_with_generation(
    cache_path: &Path,
    notifications: &[Notification],
    options_hash: &str,
    generation: &str,
) -> Result<()> {
    let cache = NotificationCache {
        version: CACHE_VERSION,
        timestamp: Utc::now(),
        fetch_options_hash: options_hash.to_string(),
        generation: generation.to_string(),
        notifications: notifications.to_vec(),
    };

    let json = serde_json::to_string(&cache)
        .map_err(|e| Error::Config(format!("Failed to serialise cache: {}", e)))?;

    write_atomically(cache_path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn fresh_cache_requires_matching_generation() {
        let dir = test_dir();
        let path = dir.join("cache.json");
        let ttl = chrono::Duration::seconds(60);
        let notifications = vec![sample_notification()];

        save_cache_with_generation(&path, &notifications, "hash", "a").unwrap();

        assert!(load_cache_fresh(&path, "hash", ttl, "a").is_some());
        assert!(load_cache_fresh(&path, "hash", ttl, "b").is_none());
        assert!(load_cache_fresh(&path, "hash", ttl, "").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_cache_accepts_an_empty_inbox() {
        let dir = test_dir();
        let path = dir.join("cache.json");
        let ttl = chrono::Duration::seconds(60);

        save_cache_with_generation(&path, &[], "hash", "").unwrap();

        assert!(load_cache_fresh(&path, "hash", ttl, "").is_some_and(|n| n.is_empty()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fetch_started_before_a_bump_is_not_served_as_fresh() {
        let dir = test_dir();
        let cache = dir.join("cache.json");
        let marker = dir.join("generation");
        let ttl = chrono::Duration::seconds(60);

        let before_fetch = read_generation(&marker);
        bump_generation(&marker);
        save_cache_with_generation(&cache, &[], "hash", &before_fetch).unwrap();

        assert!(load_cache_fresh(&cache, "hash", ttl, &read_generation(&marker)).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bumping_changes_the_token_each_time() {
        let dir = test_dir();
        let marker = dir.join("generation");

        assert_eq!(read_generation(&marker), "");
        bump_generation(&marker);
        let first = read_generation(&marker);
        bump_generation(&marker);

        assert!(!first.is_empty());
        assert_ne!(first, read_generation(&marker));

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
