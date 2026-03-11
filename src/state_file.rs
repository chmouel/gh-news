use crate::error::{Error, Result};
use crate::models::Notification;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    #[serde(default = "default_auto_mark_read")]
    pub auto_mark_read: bool,
    #[serde(default)]
    pub auto_archive: bool,
    #[serde(default)]
    pub pinned_notifications: Vec<Notification>,
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

    fn update_with<F>(mut update: F) -> Result<()>
    where
        F: FnMut(&mut AppStateFile),
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
}
