use crate::error::{Error, Result};
use crate::state::PreviewMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    pub preview_mode: String,
}

impl AppStateFile {
    pub fn new(preview_mode: PreviewMode) -> Self {
        Self {
            preview_mode: preview_mode_to_string(preview_mode),
        }
    }

    pub fn get_preview_mode(&self) -> PreviewMode {
        string_to_preview_mode(&self.preview_mode)
    }

    pub fn get_state_path() -> Result<PathBuf> {
        // Use XDG cache directory
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?;

        // Create gh-news directory if it doesn't exist
        let state_dir = cache_dir.join("gh-news");
        fs::create_dir_all(&state_dir).map_err(Error::Io)?;

        Ok(state_dir.join("state.toml"))
    }

    pub fn save(preview_mode: PreviewMode) -> Result<()> {
        let state = Self::new(preview_mode);
        let path = Self::get_state_path()?;

        let toml_content = toml::to_string_pretty(&state)
            .map_err(|e| Error::Config(format!("Failed to serialize state: {}", e)))?;

        fs::write(&path, toml_content).map_err(Error::Io)?;

        Ok(())
    }

    pub fn load() -> Result<PreviewMode> {
        let path = Self::get_state_path()?;

        if !path.exists() {
            // Return default if file doesn't exist
            return Ok(PreviewMode::Horizontal);
        }

        let content = fs::read_to_string(&path).map_err(Error::Io)?;

        let state: AppStateFile = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse state: {}", e)))?;

        Ok(state.get_preview_mode())
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
