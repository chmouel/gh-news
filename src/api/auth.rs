use crate::error::{AuthError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

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

pub fn get_github_token(github_host: &str) -> Result<String> {
    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    if let Some(token) = token_from_gh_config() {
        return Ok(token);
    }

    if let Some(token) = token_from_gh_cli(github_host) {
        return Ok(token);
    }

    Err(AuthError::TokenNotFound.into())
}

fn token_from_gh_config() -> Option<String> {
    let config_path = get_gh_config_path().ok()?;
    if !config_path.exists() {
        return None;
    }

    let config_content = std::fs::read_to_string(&config_path).ok()?;
    let config: HashMap<String, HostConfig> = serde_yaml::from_str(&config_content).ok()?;

    let find_token = |token: Option<&String>| token.filter(|t| !t.is_empty()).cloned();

    let token_from_host = |host_config: &HostConfig| -> Option<String> {
        if let Some(token) = find_token(host_config.oauth_token.as_ref()) {
            return Some(token);
        }

        if let (Some(user), Some(users)) = (&host_config.user, &host_config.users) {
            if let Some(user_config) = users.get(user) {
                if let Some(token) = find_token(user_config.oauth_token.as_ref()) {
                    return Some(token);
                }
            }
        }

        if let Some(users) = &host_config.users {
            for user_config in users.values() {
                if let Some(token) = find_token(user_config.oauth_token.as_ref()) {
                    return Some(token);
                }
            }
        }

        None
    };

    if let Some(host_config) = config.get("github.com") {
        if let Some(token) = token_from_host(host_config) {
            return Some(token);
        }
    }

    for host_config in config.values() {
        if let Some(token) = token_from_host(host_config) {
            return Some(token);
        }
    }

    None
}

/// Retrieve a token via `gh auth token`, which reads from the system keyring
/// on modern gh CLI versions. Returns None if gh is not installed or not
/// authenticated.
fn token_from_gh_cli(github_host: &str) -> Option<String> {
    let mut cmd = Command::new("gh");
    cmd.arg("auth").arg("token");
    if github_host != "github.com" {
        cmd.arg("-h").arg(github_host);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8(output.stdout).ok()?;
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    Some(token.to_string())
}

fn get_gh_config_path() -> Result<PathBuf> {
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_path = config_dir.join("gh").join("hosts.yml");
        if xdg_path.exists() {
            return Ok(xdg_path);
        }
    }

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
        let token = get_github_token("github.com").unwrap();
        assert_eq!(token, "test_token_123");
        std::env::remove_var("GH_TOKEN");
    }

    #[test]
    fn test_gh_cli_token_fallback() {
        // Only meaningful when gh is installed and authenticated
        let has_gh = Command::new("gh")
            .arg("auth")
            .arg("token")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_gh {
            return;
        }

        std::env::remove_var("GH_TOKEN");
        std::env::remove_var("GITHUB_TOKEN");
        let token = token_from_gh_cli("github.com");
        assert!(token.is_some(), "gh auth token should return a token");
        assert!(!token.unwrap().is_empty());
    }

    #[test]
    fn test_gh_cli_with_unknown_host() {
        let token = token_from_gh_cli("nonexistent.example.com");
        assert!(token.is_none());
    }
}
