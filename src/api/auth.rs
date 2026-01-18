use crate::error::{AuthError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// The actual gh config format is:
// github.com:
//   oauth_token: ...
//   users:
//     username:
//       oauth_token: ...

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostConfig {
    #[serde(default)]
    oauth_token: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    users: Option<HashMap<String, UserConfig>>,
    #[serde(rename = "git_protocol")]
    #[serde(default)]
    git_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserConfig {
    #[serde(default)]
    oauth_token: Option<String>,
}

pub fn get_github_token() -> Result<String> {
    // First, try environment variable
    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Then try GITHUB_TOKEN
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Finally, try reading from gh config
    let config_path = get_gh_config_path()?;
    if !config_path.exists() {
        return Err(AuthError::TokenNotFound.into());
    }

    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| AuthError::ConfigReadFailed(format!("{}: {}", config_path.display(), e)))?;

    // Parse as a map of host -> config
    let config: HashMap<String, HostConfig> = serde_yaml::from_str(&config_content)
        .map_err(|e| AuthError::ConfigParseFailed(format!("Failed to parse YAML: {}", e)))?;

    // Helper to find first non-empty token
    let find_token = |token: Option<&String>| token.filter(|t| !t.is_empty()).cloned();

    // Try to get token from github.com host first
    if let Some(host_config) = config.get("github.com") {
        // Try top-level oauth_token first
        if let Some(token) = find_token(host_config.oauth_token.as_ref()) {
            return Ok(token);
        }

        // Try user-specific token if user is set
        if let (Some(user), Some(users)) = (&host_config.user, &host_config.users) {
            if let Some(user_config) = users.get(user) {
                if let Some(token) = find_token(user_config.oauth_token.as_ref()) {
                    return Ok(token);
                }
            }
        }

        // Try any user's token as fallback
        if let Some(users) = &host_config.users {
            for user_config in users.values() {
                if let Some(token) = find_token(user_config.oauth_token.as_ref()) {
                    return Ok(token);
                }
            }
        }
    }

    // Try any host as last resort
    for host_config in config.values() {
        if let Some(token) = find_token(host_config.oauth_token.as_ref()) {
            return Ok(token);
        }
    }

    Err(AuthError::TokenNotFound.into())
}

fn get_gh_config_path() -> Result<PathBuf> {
    // Try XDG config first
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_path = config_dir.join("gh").join("hosts.yml");
        if xdg_path.exists() {
            return Ok(xdg_path);
        }
    }

    // Fallback to ~/.config/gh/hosts.yml
    dirs::home_dir()
        .map(|home| home.join(".config").join("gh").join("hosts.yml"))
        .ok_or_else(|| {
            AuthError::ConfigReadFailed("Could not determine config directory".to_string()).into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_token() {
        std::env::set_var("GH_TOKEN", "test_token_123");
        let token = get_github_token().unwrap();
        assert_eq!(token, "test_token_123");
        std::env::remove_var("GH_TOKEN");
    }
}
