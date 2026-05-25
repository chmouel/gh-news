use crate::api::GitHubClient;
use crate::config::{Action, Config};
use crate::error::{Error, Result};
use crate::filter::Filter;
use regex::Regex;

const SINGULAR_PLACEHOLDERS: &[&str] = &[
    "id",
    "title",
    "number",
    "url",
    "repo",
    "owner",
    "full_name",
    "type",
    "reason",
    "unread",
];

const BATCH_PLACEHOLDERS: &[&str] = &[
    "ids",
    "titles",
    "urls",
    "repos",
    "owners",
    "full_names",
    "types",
    "reasons",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Check {
    status: CheckStatus,
    message: String,
}

impl Check {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Ok,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Warning,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Error,
            message: message.into(),
        }
    }
}

pub fn run(config: &Config) -> Result<()> {
    let checks = collect_checks(config);
    print_checks(&checks);

    if checks
        .iter()
        .any(|check| matches!(check.status, CheckStatus::Error))
    {
        return Err(Error::Config(
            "configuration check found problems".to_string(),
        ));
    }

    Ok(())
}

fn collect_checks(config: &Config) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(
        match Filter::from_config(config.default_filter.as_deref(), config) {
            Ok(_) => Check::ok("Global filters are valid"),
            Err(err) => Check::error(format!("Global filters are invalid: {err}")),
        },
    );

    for view in &config.views {
        checks.push(
            match Filter::from_view(view, config.default_filter.as_deref(), config) {
                Ok(_) => Check::ok(format!("View '{}' is valid", view.name)),
                Err(err) => Check::error(format!("View '{}' is invalid: {err}", view.name)),
            },
        );
    }

    for action in &config.actions {
        checks.extend(validate_action(action));
    }

    checks.push(match GitHubClient::new(config) {
        Ok(client) => match client.get_authenticated_user() {
            Ok(login) => Check::ok(format!("GitHub authentication works for @{login}")),
            Err(err) => Check::error(format!("GitHub authentication failed: {err}")),
        },
        Err(err) => Check::error(format!("GitHub client could not start: {err}")),
    });

    checks
}

fn validate_action(action: &Action) -> Vec<Check> {
    let mut checks = Vec::new();

    if action.name.trim().is_empty() {
        checks.push(Check::error("An action has an empty name"));
    }

    if action.command.trim().is_empty() {
        checks.push(Check::error(format!(
            "Action '{}' has an empty command",
            action.name
        )));
    }

    if action.interactive && action.show_output {
        checks.push(Check::error(format!(
            "Action '{}' cannot be both interactive and show_output",
            action.name
        )));
    }

    for placeholder in unknown_placeholders(&action.command) {
        checks.push(Check::warning(format!(
            "Action '{}' uses unknown placeholder {{{}}}",
            action.name, placeholder
        )));
    }

    if checks.is_empty() {
        checks.push(Check::ok(format!("Action '{}' is valid", action.name)));
    }

    checks
}

fn unknown_placeholders(command: &str) -> Vec<String> {
    let placeholder_re = Regex::new(r"\{([a-z_]+)\}").expect("placeholder regex is valid");
    placeholder_re
        .captures_iter(command)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .filter(|name| {
            !SINGULAR_PLACEHOLDERS.contains(&name.as_str())
                && !BATCH_PLACEHOLDERS.contains(&name.as_str())
        })
        .collect()
}

fn print_checks(checks: &[Check]) {
    println!("gh-news configuration check");
    for check in checks {
        let label = match check.status {
            CheckStatus::Ok => "OK",
            CheckStatus::Warning => "WARN",
            CheckStatus::Error => "FAIL",
        };
        println!("[{label}] {}", check.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_placeholders_detects_unsupported_names() {
        assert_eq!(
            unknown_placeholders("echo {url} {unknown} {full_name}"),
            vec!["unknown".to_string()]
        );
    }

    #[test]
    fn action_rejects_interactive_output_combo() {
        let action = Action {
            name: "Bad".to_string(),
            command: "echo {url}".to_string(),
            priority: None,
            interactive: true,
            show_output: true,
        };

        assert!(validate_action(&action)
            .iter()
            .any(|check| check.status == CheckStatus::Error));
    }
}
