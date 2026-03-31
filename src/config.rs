use crate::error::{Error, Result};
use crate::state::{OrgGroupingMode, PreviewMode};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// A user-defined action that can be executed on notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Display name shown in the action menu.
    pub name: String,
    /// Command template with placeholders like {url}, {title}, {repo}, etc.
    pub command: String,
    /// If true, suspend TUI and run command interactively with full terminal access.
    /// This allows running TUI-based tools like fzf, vim, etc. Default: false.
    #[serde(default)]
    pub interactive: bool,
    /// If true, capture command output and display it in a scrollable TUI popup.
    /// Incompatible with `interactive`. Default: false.
    #[serde(default)]
    pub show_output: bool,
}

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
    pub org_grouping: OrgGroupingMode,

    // Behaviour
    pub auto_mark_read: bool,
    pub auto_archive: bool,

    // External commands
    pub browser_command: Option<String>,

    // Notification hooks
    pub on_new_notification_command: Option<String>,

    // GitHub Enterprise support
    pub github_host: String,

    // Notification cache
    pub cache_file: Option<String>,

    // State file path (optional override)
    pub state_file: Option<String>,

    // Structured notification filters
    /// Exclude notifications of these types (e.g., ["CheckSuite", "Release"]).
    #[serde(default)]
    pub exclude_types: Vec<String>,
    /// Exclude notifications for these reasons (e.g., ["subscribed", "ci_activity"]).
    #[serde(default)]
    pub exclude_reasons: Vec<String>,
    /// Exclude notifications from these repos. Supports glob patterns (e.g., ["org/*"]).
    #[serde(default)]
    pub exclude_repos: Vec<String>,
    /// Exclude notifications whose title matches any of these regex patterns.
    #[serde(default)]
    pub exclude_subjects: Vec<String>,

    // User-defined actions for notifications
    #[serde(default)]
    pub actions: Vec<Action>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_refresh_interval: 120,
            api_timeout: 30,
            max_notifications: None,
            pagination_size: 50,
            show_read: false,
            participating_only: false,
            default_filter: None,
            default_preview_mode: "vertical".to_string(),
            repos_collapsed: false,
            org_grouping: OrgGroupingMode::default(),
            auto_mark_read: true,
            auto_archive: false,
            cache_file: None,
            browser_command: None,
            on_new_notification_command: None,
            github_host: "github.com".to_string(),
            state_file: None,
            exclude_types: Vec::new(),
            exclude_reasons: Vec::new(),
            exclude_repos: Vec::new(),
            exclude_subjects: Vec::new(),
            actions: Vec::new(),
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

    /// Get the default config file path (~/.config/gh-news/config.toml).
    pub fn get_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Config("Could not determine home directory".to_string()))?;

        Ok(home.join(".config").join("gh-news").join("config.toml"))
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
        assert_eq!(config.auto_refresh_interval, 120);
        assert_eq!(config.api_timeout, 30);
        assert_eq!(config.pagination_size, 50);
        assert!(!config.show_read);
        assert!(!config.participating_only);
        assert!(matches!(config.org_grouping, OrgGroupingMode::Auto));
        assert_eq!(config.github_host, "github.com");
    }

    #[test]
    fn test_github_api_base() {
        let config = Config::default();
        assert_eq!(config.github_api_base(), "https://api.github.com");

        let enterprise_config = Config {
            github_host: "github.example.com".to_string(),
            ..Default::default()
        };
        assert_eq!(
            enterprise_config.github_api_base(),
            "https://github.example.com/api/v3"
        );
    }

    #[test]
    fn test_preview_mode_parsing() {
        let mut config = Config {
            default_preview_mode: "off".to_string(),
            ..Default::default()
        };
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

    #[test]
    fn test_default_actions_empty() {
        let config = Config::default();
        assert!(config.actions.is_empty());
    }

    #[test]
    fn test_parse_actions_from_toml() {
        let toml_str = r#"
[[actions]]
name = "Copy URL"
command = "echo {url}"

[[actions]]
name = "Interactive action"
command = "vim {title}"
interactive = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.actions.len(), 2);

        assert_eq!(config.actions[0].name, "Copy URL");
        assert_eq!(config.actions[0].command, "echo {url}");
        assert!(!config.actions[0].interactive);

        assert_eq!(config.actions[1].name, "Interactive action");
        assert_eq!(config.actions[1].command, "vim {title}");
        assert!(config.actions[1].interactive);
    }

    #[test]
    fn test_action_interactive_defaults_to_false() {
        let toml_str = r#"
[[actions]]
name = "Test"
command = "echo test"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.actions[0].interactive);
    }

    #[test]
    fn test_parse_single_action() {
        let toml_str = r#"
[[actions]]
name = "Single"
command = "true"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.actions.len(), 1);
    }

    #[test]
    fn test_parse_exclude_filters_from_toml() {
        let toml_str = r#"
exclude_types = ["CheckSuite", "Release"]
exclude_reasons = ["subscribed", "ci_activity"]
exclude_repos = ["org/*", "some-org/noisy-repo"]
exclude_subjects = ["\\[bot\\]", "^Bump "]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.exclude_types, vec!["CheckSuite", "Release"]);
        assert_eq!(config.exclude_reasons, vec!["subscribed", "ci_activity"]);
        assert_eq!(config.exclude_repos, vec!["org/*", "some-org/noisy-repo"]);
        assert_eq!(config.exclude_subjects, vec!["\\[bot\\]", "^Bump "]);
    }

    #[test]
    fn test_exclude_filters_default_empty() {
        let config = Config::default();
        assert!(config.exclude_types.is_empty());
        assert!(config.exclude_reasons.is_empty());
        assert!(config.exclude_repos.is_empty());
        assert!(config.exclude_subjects.is_empty());
    }

    #[test]
    fn test_parse_action_with_placeholders() {
        let toml_str = r#"
[[actions]]
name = "Full"
command = "{id} {title} {url} {repo} {owner}"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.actions[0].command.contains("{id}"));
        assert!(config.actions[0].command.contains("{title}"));
        assert!(config.actions[0].command.contains("{url}"));
    }
}
