use crate::api::GitHubClient;
use crate::models::Notification;
use crate::preview::{PreviewData, PreviewFetcher};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Fetch request priority.  Lower value = higher urgency.
pub const PRIORITY_HIGH: u8 = 0;
/// Background prefetch and revalidation priority.
pub const PRIORITY_LOW: u8 = 1;

#[derive(Debug, Clone)]
struct CachedPreview {
    data: PreviewData,
    updated_at: Option<DateTime<Utc>>,
}

/// Freshness status of a cached preview relative to the current notification state.
#[derive(Debug, Clone)]
pub enum CacheStatus {
    /// Cached and up-to-date with the notification's `updated_at` timestamp.
    Fresh(PreviewData),
    /// Cached but the notification has been updated since the entry was populated.
    Stale(PreviewData),
    /// No cached entry exists.
    Miss,
}

#[derive(Debug)]
struct FetchRequest {
    notification_id: String,
    notification: Notification,
    /// Generation token; a completed result is discarded if it no longer matches.
    generation: u64,
    /// When true, fetch even if cached data already exists (revalidation).
    force: bool,
    /// 0 = high priority (user-visible), 1 = low priority (background prefetch).
    priority: u8,
}

// `BinaryHeap` requires `Ord`.  We compare only on the fields that determine
// scheduling order; `notification` is deliberately excluded because `Notification`
// does not implement `Eq`.
//
// `BinaryHeap` is a max-heap.  We want lower `priority` numbers to pop first
// (higher urgency), so we reverse the comparison on `priority`.
impl PartialEq for FetchRequest {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
            && self.generation == other.generation
            && self.notification_id == other.notification_id
    }
}

impl Eq for FetchRequest {}

impl Ord for FetchRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.generation.cmp(&self.generation))
            .then_with(|| self.notification_id.cmp(&other.notification_id))
    }
}

impl PartialOrd for FetchRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
struct FetchResult {
    notification_id: String,
}

pub struct PreviewManager {
    cache: Arc<Mutex<HashMap<String, CachedPreview>>>,
    loading: Arc<Mutex<HashSet<String>>>,
    /// Per-notification-id generation counter.  Incrementing invalidates in-flight requests.
    generation: Arc<Mutex<HashMap<String, u64>>>,
    tx: Sender<FetchRequest>,
    rx: Receiver<FetchResult>,
    _thread: JoinHandle<()>,
}

fn preview_worker_thread(
    client: GitHubClient,
    rx: Receiver<FetchRequest>,
    tx: Sender<FetchResult>,
    cache: Arc<Mutex<HashMap<String, CachedPreview>>>,
    loading: Arc<Mutex<HashSet<String>>>,
    generation: Arc<Mutex<HashMap<String, u64>>>,
) {
    let mut heap: BinaryHeap<FetchRequest> = BinaryHeap::new();

    loop {
        // Block until at least one request arrives when the heap is empty.
        if heap.is_empty() {
            match rx.recv() {
                Ok(req) => heap.push(req),
                Err(_) => break, // sender dropped; exit thread
            }
        }

        // Drain all currently queued requests into the heap (non-blocking).
        // This allows any high-priority request that arrived while we were
        // fetching to jump ahead of pending low-priority work.
        loop {
            match rx.try_recv() {
                Ok(req) => heap.push(req),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        let request = match heap.pop() {
            Some(r) => r,
            None => continue,
        };

        // Skip fetch if already cached and this is not a forced revalidation.
        if !request.force {
            let cache_lock = cache.lock();
            if cache_lock.contains_key(&request.notification_id) {
                loading.lock().remove(&request.notification_id);
                let _ = tx.send(FetchResult {
                    notification_id: request.notification_id,
                });
                continue;
            }
        }

        // Pre-flight generation check — skip wasted work before the HTTP call.
        let current_gen = generation
            .lock()
            .get(&request.notification_id)
            .copied()
            .unwrap_or(0);
        if current_gen != request.generation {
            loading.lock().remove(&request.notification_id);
            let _ = tx.send(FetchResult {
                notification_id: request.notification_id,
            });
            continue;
        }

        let result = PreviewFetcher::fetch_preview(&client, &request.notification)
            .map_err(|e| e.to_string());

        // Post-flight generation check — discard result if superseded while fetching.
        let current_gen = generation
            .lock()
            .get(&request.notification_id)
            .copied()
            .unwrap_or(0);
        if current_gen != request.generation {
            loading.lock().remove(&request.notification_id);
            let _ = tx.send(FetchResult {
                notification_id: request.notification_id,
            });
            continue;
        }

        let notification_updated_at = request.notification.updated_at;
        let data = match result {
            Ok(d) => d,
            Err(error) => PreviewData::Generic {
                title: request.notification.title().to_string(),
                body: format!("Error loading details\n\n{}", error),
            },
        };
        cache.lock().insert(
            request.notification_id.clone(),
            CachedPreview {
                data,
                updated_at: notification_updated_at,
            },
        );

        loading.lock().remove(&request.notification_id);
        let _ = tx.send(FetchResult {
            notification_id: request.notification_id,
        });
    }
}

impl PreviewManager {
    pub fn new(client: GitHubClient) -> Self {
        let (tx, rx_worker) = channel::<FetchRequest>();
        let (result_tx, result_rx) = channel::<FetchResult>();
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let loading = Arc::new(Mutex::new(HashSet::new()));
        let generation = Arc::new(Mutex::new(HashMap::new()));

        let thread_cache = Arc::clone(&cache);
        let thread_loading = Arc::clone(&loading);
        let thread_generation = Arc::clone(&generation);
        let thread_client = client.clone();

        let handle = thread::spawn(move || {
            preview_worker_thread(
                thread_client,
                rx_worker,
                result_tx,
                thread_cache,
                thread_loading,
                thread_generation,
            );
        });

        Self {
            cache,
            loading,
            generation,
            tx,
            rx: result_rx,
            _thread: handle,
        }
    }

    pub fn get_cached(&self, notification_id: &str) -> Option<PreviewData> {
        self.cache
            .lock()
            .get(notification_id)
            .map(|cached| cached.data.clone())
    }

    /// Returns the freshness status of the cached preview for the given notification.
    pub fn get_cached_status(&self, notification: &Notification) -> CacheStatus {
        let cache = self.cache.lock();
        match cache.get(&notification.id) {
            None => CacheStatus::Miss,
            Some(cached) => {
                let is_stale = match (notification.updated_at, cached.updated_at) {
                    (Some(new_ts), Some(old_ts)) => new_ts > old_ts,
                    (Some(_), None) => true,
                    _ => false,
                };
                if is_stale {
                    CacheStatus::Stale(cached.data.clone())
                } else {
                    CacheStatus::Fresh(cached.data.clone())
                }
            }
        }
    }

    pub fn is_loading(&self, notification_id: &str) -> bool {
        self.loading.lock().contains(notification_id)
    }

    /// Queue a fetch for `notification` at the given priority, skipping if already
    /// cached or in-flight.
    pub fn request_preview(&self, notification: &Notification, priority: u8) {
        let notification_id = notification.id.clone();
        if self.cache.lock().contains_key(&notification_id) {
            return;
        }
        if self.loading.lock().contains(&notification_id) {
            return;
        }

        let gen = self
            .generation
            .lock()
            .get(&notification_id)
            .copied()
            .unwrap_or(0);
        self.loading.lock().insert(notification_id.clone());
        let _ = self.tx.send(FetchRequest {
            notification_id,
            notification: notification.clone(),
            generation: gen,
            force: false,
            priority,
        });
    }

    /// Force a fresh fetch regardless of cache state, at the given priority.
    ///
    /// Increments the per-id generation counter so any in-flight older fetch is
    /// discarded when it completes.  No-op if the id is already loading (the
    /// in-flight request will complete and populate the cache shortly).
    pub fn request_revalidation(&self, notification: &Notification, priority: u8) {
        let notification_id = notification.id.clone();
        if self.loading.lock().contains(&notification_id) {
            return;
        }

        let gen = {
            let mut gen_lock = self.generation.lock();
            let entry = gen_lock.entry(notification_id.clone()).or_insert(0);
            *entry += 1;
            *entry
        };

        self.loading.lock().insert(notification_id.clone());
        let _ = self.tx.send(FetchRequest {
            notification_id,
            notification: notification.clone(),
            generation: gen,
            force: true,
            priority,
        });
    }

    /// Check which notifications have stale cached previews, bump their generation
    /// (so in-flight old fetches are discarded), and clear them from `loading` so
    /// revalidation can be immediately re-queued.
    ///
    /// Cached data is kept in place for stale-while-revalidate display.
    ///
    /// Returns the set of invalidated notification IDs.
    pub fn invalidate_notifications(&self, notifications: &[Notification]) -> HashSet<String> {
        let cache = self.cache.lock();
        let mut gen_lock = self.generation.lock();
        let mut loading_lock = self.loading.lock();
        let mut invalidated = HashSet::new();

        for notification in notifications {
            if let Some(cached) = cache.get(&notification.id) {
                let is_stale = match (notification.updated_at, cached.updated_at) {
                    (Some(new_ts), Some(old_ts)) => new_ts > old_ts,
                    (Some(_), None) => true,
                    _ => false,
                };
                if is_stale {
                    let entry = gen_lock.entry(notification.id.clone()).or_insert(0);
                    *entry += 1;
                    // Clear from loading so callers can immediately re-queue revalidation.
                    loading_lock.remove(&notification.id);
                    invalidated.insert(notification.id.clone());
                }
            }
        }

        invalidated
    }

    /// Queue low-priority background fetches for all notifications that are neither
    /// cached nor already loading.
    ///
    /// Intended to be called once after initial load to warm the cache for all
    /// notifications in the inbox.  Requests are processed after any higher-priority
    /// user-visible requests already in the worker's queue.
    pub fn prefetch_all(&self, notifications: &[Notification]) {
        for notification in notifications {
            let id = &notification.id;
            if self.cache.lock().contains_key(id) || self.loading.lock().contains(id) {
                continue;
            }
            let gen = self.generation.lock().get(id).copied().unwrap_or(0);
            self.loading.lock().insert(id.clone());
            let _ = self.tx.send(FetchRequest {
                notification_id: id.clone(),
                notification: notification.clone(),
                generation: gen,
                force: false,
                priority: PRIORITY_LOW,
            });
        }
    }

    /// Queue low-priority background revalidation for every notification whose
    /// cached entry is stale, skipping `skip_id` (which the caller handles at
    /// high priority).
    ///
    /// Intended to be called after `invalidate_notifications` so that all stale
    /// entries are refreshed proactively, not just the one the user is looking at.
    pub fn revalidate_all_stale(&self, notifications: &[Notification], skip_id: Option<&str>) {
        for notification in notifications {
            if skip_id == Some(notification.id.as_str()) {
                continue;
            }
            if matches!(self.get_cached_status(notification), CacheStatus::Stale(_)) {
                self.request_revalidation(notification, PRIORITY_LOW);
            }
        }
    }

    pub fn drain_completed(&self) -> Vec<String> {
        let mut completed = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(result) => completed.push(result.notification_id),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Notification, NotificationType};
    use chrono::TimeZone;

    fn make_notification(id: &str, updated_at: Option<DateTime<Utc>>) -> Notification {
        Notification {
            id: id.to_string(),
            unread: true,
            last_read_at: None,
            updated_at,
            reason: "subscribed".to_string(),
            repository: crate::models::Repository {
                id: 1,
                name: "repo".to_string(),
                full_name: "owner/repo".to_string(),
                owner: crate::models::Owner {
                    login: "owner".to_string(),
                    id: 1,
                    owner_type: "User".to_string(),
                },
                private: false,
            },
            subject: crate::models::Subject {
                title: "Test notification".to_string(),
                subject_type: NotificationType::Issue,
                url: None,
                latest_comment_url: None,
            },
            latest_comment_url: None,
            author: None,
            context: None,
        }
    }

    fn make_cached(updated_at: Option<DateTime<Utc>>) -> CachedPreview {
        CachedPreview {
            data: PreviewData::Generic {
                title: "t".into(),
                body: "b".into(),
            },
            updated_at,
        }
    }

    fn ts(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    fn manager_with_cache(entries: Vec<(&str, CachedPreview)>) -> PreviewManager {
        let client = crate::api::GitHubClient::new_test();
        let pm = PreviewManager::new(client);
        {
            let mut cache = pm.cache.lock();
            for (id, entry) in entries {
                cache.insert(id.to_string(), entry);
            }
        }
        pm
    }

    // ── CacheStatus tests ────────────────────────────────────────────────────

    #[test]
    fn fresh_entry_is_reported_fresh() {
        let old = ts(2024, 1, 1);
        let pm = manager_with_cache(vec![("42", make_cached(Some(old)))]);
        let notif = make_notification("42", Some(old));
        assert!(matches!(
            pm.get_cached_status(&notif),
            CacheStatus::Fresh(_)
        ));
    }

    #[test]
    fn newer_notification_makes_cached_entry_stale() {
        let cached_ts = ts(2024, 1, 1);
        let newer_ts = ts(2024, 6, 1);
        let pm = manager_with_cache(vec![("42", make_cached(Some(cached_ts)))]);
        let notif = make_notification("42", Some(newer_ts));
        assert!(matches!(
            pm.get_cached_status(&notif),
            CacheStatus::Stale(_)
        ));
    }

    #[test]
    fn missing_entry_is_miss() {
        let pm = manager_with_cache(vec![]);
        let notif = make_notification("99", Some(ts(2024, 1, 1)));
        assert!(matches!(pm.get_cached_status(&notif), CacheStatus::Miss));
    }

    // ── invalidate_notifications tests ───────────────────────────────────────

    #[test]
    fn invalidate_notifications_marks_stale_ids() {
        let cached_ts = ts(2024, 1, 1);
        let newer_ts = ts(2024, 6, 1);
        let pm = manager_with_cache(vec![
            ("stale", make_cached(Some(cached_ts))),
            ("fresh", make_cached(Some(newer_ts))),
        ]);

        let notifications = vec![
            make_notification("stale", Some(newer_ts)),
            make_notification("fresh", Some(newer_ts)),
        ];

        let invalidated = pm.invalidate_notifications(&notifications);
        assert!(invalidated.contains("stale"));
        assert!(!invalidated.contains("fresh"));

        let gen = pm.generation.lock();
        assert_eq!(gen.get("stale").copied().unwrap_or(0), 1);
        assert_eq!(gen.get("fresh").copied().unwrap_or(0), 0);
    }

    #[test]
    fn invalidate_keeps_cached_data_for_stale_while_revalidate() {
        let cached_ts = ts(2024, 1, 1);
        let newer_ts = ts(2024, 6, 1);
        let pm = manager_with_cache(vec![("42", make_cached(Some(cached_ts)))]);

        let notif = make_notification("42", Some(newer_ts));
        pm.invalidate_notifications(std::slice::from_ref(&notif));

        assert!(pm.cache.lock().contains_key("42"));
        assert!(matches!(
            pm.get_cached_status(&notif),
            CacheStatus::Stale(_)
        ));
    }

    #[test]
    fn invalidate_notifications_clears_loading_for_stale_ids() {
        let cached_ts = ts(2024, 1, 1);
        let newer_ts = ts(2024, 6, 1);
        let pm = manager_with_cache(vec![("stale", make_cached(Some(cached_ts)))]);
        // Simulate an in-flight fetch for "stale".
        pm.loading.lock().insert("stale".to_string());

        let notifications = vec![make_notification("stale", Some(newer_ts))];
        pm.invalidate_notifications(&notifications);

        // Loading flag must be cleared so revalidation can be re-queued immediately.
        assert!(!pm.is_loading("stale"));
        assert_eq!(pm.generation.lock().get("stale").copied().unwrap_or(0), 1);
    }

    // ── revalidation tests ───────────────────────────────────────────────────

    #[test]
    fn request_revalidation_bumps_generation() {
        let cached_ts = ts(2024, 1, 1);
        let pm = manager_with_cache(vec![("42", make_cached(Some(cached_ts)))]);

        let notif = make_notification("42", Some(cached_ts));
        pm.request_revalidation(&notif, PRIORITY_HIGH);

        assert_eq!(pm.generation.lock().get("42").copied().unwrap_or(0), 1);
        assert!(pm.is_loading("42"));
    }

    #[test]
    fn request_revalidation_is_noop_when_already_loading() {
        let pm = manager_with_cache(vec![]);
        pm.loading.lock().insert("42".to_string());

        let notif = make_notification("42", None);
        pm.request_revalidation(&notif, PRIORITY_HIGH);

        assert_eq!(pm.generation.lock().get("42").copied().unwrap_or(0), 0);
    }

    // ── priority queue tests ─────────────────────────────────────────────────

    #[test]
    fn high_priority_sorts_before_low_priority() {
        let high = FetchRequest {
            notification_id: "a".into(),
            notification: make_notification("a", None),
            generation: 0,
            force: false,
            priority: PRIORITY_HIGH,
        };
        let low = FetchRequest {
            notification_id: "b".into(),
            notification: make_notification("b", None),
            generation: 0,
            force: false,
            priority: PRIORITY_LOW,
        };
        let mut heap = BinaryHeap::new();
        heap.push(low);
        heap.push(high);
        // High priority (0) must come out first from the max-heap.
        assert_eq!(heap.pop().unwrap().notification_id, "a");
        assert_eq!(heap.pop().unwrap().notification_id, "b");
    }

    // ── prefetch_all tests ───────────────────────────────────────────────────

    #[test]
    fn prefetch_all_skips_cached_and_loading() {
        let ts_val = ts(2024, 1, 1);
        let pm = manager_with_cache(vec![("cached", make_cached(Some(ts_val)))]);
        pm.loading.lock().insert("loading_id".to_string());

        let notifs = vec![
            make_notification("cached", Some(ts_val)),
            make_notification("loading_id", Some(ts_val)),
            make_notification("new_id", Some(ts_val)),
        ];
        pm.prefetch_all(&notifs);

        // Only "new_id" should have been enqueued.
        assert!(pm.is_loading("new_id"));
        // Others must be untouched.
        assert!(pm.cache.lock().contains_key("cached"));
        // "loading_id" was already loading; it must still only appear once.
        assert!(pm.is_loading("loading_id"));
    }
}
