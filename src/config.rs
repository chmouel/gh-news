use crate::error::{Error, Result};
use crate::state::PreviewMode;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration loaded from config file and environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // API & Network settings
    pub auto_refresh_interval: u64,
    pub api_timeout: u64,
    pub max_notifications: Option<usize>,
    pub pagination_size: usize,

    // Default filters (CLI flag defaults)
    pub show_read: bool,
    pub participating_only: bool,
    pub default_filter: Option<String>,

    // Display defaults
    pub default_preview_mode: String,
    pub repos_collapsed: bool,

    // Behaviour
    pub auto_mark_read: bool,

    // External commands
    pub browser_command: Option<String>,

    // Notification hooks
    pub on_new_notification_command: Option<String>,

    // GitHub Enterprise support
    pub github_host: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_refresh_interval: 300,
            api_timeout: 30,
            max_notifications: None,
            pagination_size: 50,
            show_read: false,
            participating_only: false,
            default_filter: None,
            default_preview_mode: "vertical".to_string(),
            repos_collapsed: false,
            auto_mark_read: true,
            browser_command: None,
            on_new_notification_command: None,
            github_host: "github.com".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from file and environment variables.
    /// Priority: CLI args > env vars > config file > defaults
    /// If config_path is provided, use it instead of the default path.
    pub fn load(config_path: Option<&Path>) -> Self {
        let mut config = Self::load_from_file(config_path).unwrap_or_default();

        // Environment variable overrides for backwards compatibility
        if let Ok(interval) = env::var("GH_NEWS_AUTO_REFRESH_INTERVAL") {
            if let Ok(value) = interval.parse() {
                config.auto_refresh_interval = value;
            }
        }

        config
    }

    /// Get the default config file path (XDG-compliant).
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not determine config directory".to_string()))?;

        let app_config_dir = config_dir.join("gh-news");
        Ok(app_config_dir.join("config.toml"))
    }

    /// Load configuration from the TOML config file.
    /// If config_path is provided, use it; otherwise use the default path.
    fn load_from_file(config_path: Option<&Path>) -> Option<Self> {
        let path = match config_path {
            Some(p) => p.to_path_buf(),
            None => Self::get_config_path().ok()?,
        };

        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        let config: Config = toml::from_str(&content).ok()?;
        Some(config)
    }

    /// Get the GitHub API base URL based on github_host config.
    pub fn github_api_base(&self) -> String {
        if self.github_host == "github.com" {
            "https://api.github.com".to_string()
        } else {
            format!("https://{}/api/v3", self.github_host)
        }
    }

    /// Parse the default_preview_mode string into PreviewMode enum.
    pub fn get_default_preview_mode(&self) -> PreviewMode {
        match self.default_preview_mode.to_lowercase().as_str() {
            "off" => PreviewMode::Off,
            "vertical" => PreviewMode::Vertical,
            _ => PreviewMode::Horizontal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.auto_refresh_interval, 300);
        assert_eq!(config.api_timeout, 30);
        assert_eq!(config.pagination_size, 50);
        assert!(!config.show_read);
        assert!(!config.participating_only);
        assert_eq!(config.github_host, "github.com");
    }

    #[test]
    fn test_github_api_base() {
        let config = Config::default();
        assert_eq!(config.github_api_base(), "https://api.github.com");

        let mut enterprise_config = Config::default();
        enterprise_config.github_host = "github.example.com".to_string();
        assert_eq!(
            enterprise_config.github_api_base(),
            "https://github.example.com/api/v3"
        );
    }

    #[test]
    fn test_preview_mode_parsing() {
        let mut config = Config::default();

        config.default_preview_mode = "off".to_string();
        assert!(matches!(
            config.get_default_preview_mode(),
            PreviewMode::Off
        ));

        config.default_preview_mode = "vertical".to_string();
        assert!(matches!(
            config.get_default_preview_mode(),
            PreviewMode::Vertical
        ));

        config.default_preview_mode = "horizontal".to_string();
        assert!(matches!(
            config.get_default_preview_mode(),
            PreviewMode::Horizontal
        ));

        // Invalid falls back to horizontal
        config.default_preview_mode = "invalid".to_string();
        assert!(matches!(
            config.get_default_preview_mode(),
            PreviewMode::Horizontal
        ));
    }
}
