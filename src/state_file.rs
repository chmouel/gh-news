use crate::error::{Error, Result};
use crate::models::Notification;
use crate::state::PreviewMode;
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
    pub preview_mode: String,
    #[serde(default = "default_auto_mark_read")]
    pub auto_mark_read: bool,
    #[serde(default)]
    pub pinned_notifications: Vec<Notification>,
}

fn default_auto_mark_read() -> bool {
    true
}

impl AppStateFile {
    pub fn new(preview_mode: PreviewMode, auto_mark_read: bool) -> Self {
        Self {
            preview_mode: preview_mode_to_string(preview_mode),
            auto_mark_read,
            pinned_notifications: Vec::new(),
        }
    }

    pub fn get_preview_mode(&self) -> PreviewMode {
        string_to_preview_mode(&self.preview_mode)
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

    fn load_or_default(preview_mode: PreviewMode, auto_mark_read: bool) -> AppStateFile {
        Self::load_full().unwrap_or_else(|_| Self::new(preview_mode, auto_mark_read))
    }

    fn update_with<F>(preview_mode: PreviewMode, auto_mark_read: bool, mut update: F) -> Result<()>
    where
        F: FnMut(&mut AppStateFile),
    {
        let mut state = Self::load_or_default(preview_mode, auto_mark_read);
        update(&mut state);
        state.save_full()
    }

    pub fn save(preview_mode: PreviewMode) -> Result<()> {
        Self::update_with(preview_mode, default_auto_mark_read(), |state| {
            state.preview_mode = preview_mode_to_string(preview_mode);
        })
    }

    pub fn load() -> Result<PreviewMode> {
        let state = Self::load_full()?;
        Ok(state.get_preview_mode())
    }

    pub fn save_auto_mark_read(auto_mark_read: bool) -> Result<()> {
        Self::update_with(PreviewMode::Vertical, auto_mark_read, |state| {
            state.auto_mark_read = auto_mark_read;
        })
    }

    pub fn load_auto_mark_read() -> Result<bool> {
        let state = Self::load_full()?;
        Ok(state.auto_mark_read)
    }

    pub fn save_pinned_notifications(pinned: &[Notification]) -> Result<()> {
        Self::update_with(PreviewMode::Vertical, default_auto_mark_read(), |state| {
            state.pinned_notifications = pinned.to_vec();
        })
    }

    pub fn load_pinned_notifications() -> Result<Vec<Notification>> {
        let state = Self::load_full()?;
        Ok(state.pinned_notifications)
    }
}

fn preview_mode_to_string(mode: PreviewMode) -> String {
    match mode {
        PreviewMode::Off => "off".to_string(),
        PreviewMode::Horizontal => "horizontal".to_string(),
        PreviewMode::Vertical => "vertical".to_string(),
    }
}

fn string_to_preview_mode(s: &str) -> PreviewMode {
    match s {
        "off" => PreviewMode::Off,
        "horizontal" => PreviewMode::Horizontal,
        "vertical" => PreviewMode::Vertical,
        _ => PreviewMode::Horizontal, // Default fallback
    }
}
