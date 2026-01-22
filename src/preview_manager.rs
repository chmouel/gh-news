use crate::api::GitHubClient;
use crate::models::Notification;
use crate::preview::{PreviewData, PreviewFetcher};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[derive(Debug)]
struct FetchRequest {
    notification_id: String,
    notification: Notification,
}

#[derive(Debug)]
struct FetchResult {
    notification_id: String,
}

pub struct PreviewManager {
    cache: Arc<Mutex<HashMap<String, PreviewData>>>,
    loading: Arc<Mutex<HashSet<String>>>,
    tx: Sender<FetchRequest>,
    rx: Receiver<FetchResult>,
    _thread: JoinHandle<()>,
}

fn preview_worker_thread(
    client: GitHubClient,
    rx: Receiver<FetchRequest>,
    tx: Sender<FetchResult>,
    cache: Arc<Mutex<HashMap<String, PreviewData>>>,
    loading: Arc<Mutex<HashSet<String>>>,
) {
    while let Ok(request) = rx.recv() {
        {
            let cache_lock = cache.lock();
            if cache_lock.contains_key(&request.notification_id) {
                loading.lock().remove(&request.notification_id);
                continue;
            }
        }

        let result = PreviewFetcher::fetch_preview(&client, &request.notification)
            .map_err(|e| e.to_string());

        match &result {
            Ok(data) => {
                let mut cache_lock = cache.lock();
                cache_lock.insert(request.notification_id.clone(), data.clone());
            }
            Err(error) => {
                let error_data = PreviewData::Generic {
                    title: request.notification.title().to_string(),
                    body: format!("Error loading details\n\n{}", error),
                };
                let mut cache_lock = cache.lock();
                cache_lock.insert(request.notification_id.clone(), error_data);
            }
        }

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

        let thread_cache = Arc::clone(&cache);
        let thread_loading = Arc::clone(&loading);
        let thread_client = client.clone();

        let handle = thread::spawn(move || {
            preview_worker_thread(
                thread_client,
                rx_worker,
                result_tx,
                thread_cache,
                thread_loading,
            );
        });

        Self {
            cache,
            loading,
            tx,
            rx: result_rx,
            _thread: handle,
        }
    }

    pub fn get_cached(&self, notification_id: &str) -> Option<PreviewData> {
        self.cache.lock().get(notification_id).cloned()
    }

    pub fn is_loading(&self, notification_id: &str) -> bool {
        self.loading.lock().contains(notification_id)
    }

    pub fn request_preview(&self, notification: &Notification) {
        let notification_id = notification.id.clone();
        if self.cache.lock().contains_key(&notification_id) {
            return;
        }
        if self.loading.lock().contains(&notification_id) {
            return;
        }

        self.loading.lock().insert(notification_id.clone());
        let _ = self.tx.send(FetchRequest {
            notification_id,
            notification: notification.clone(),
        });
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
