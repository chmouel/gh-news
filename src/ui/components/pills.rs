//! Pill/chip rendering helpers for the pull-request preview.
//!
//! Pills are padded, background-coloured spans in the style of
//! lazyworktree's CI chips: ` ✓ Passed ` with a contrasting foreground.

use crate::ui::theme::ColorPalette;
use ratatui::prelude::*;

/// Visual tone of a pill, mapped to palette colours at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillTone {
    Success,
    Failure,
    Pending,
    Running,
    Neutral,
    Merged,
    Info,
}

impl PillTone {
    fn colors(self, palette: &ColorPalette) -> (Color, Color) {
        // (bg, fg): coloured background with the dark base as text colour
        // keeps pills readable on both light and dark palettes.
        match self {
            PillTone::Success => (palette.green, palette.bg),
            PillTone::Failure => (palette.red, palette.bg),
            PillTone::Pending => (palette.yellow, palette.bg),
            PillTone::Running => (palette.blue, palette.bg),
            PillTone::Neutral => (palette.bg_highlight, palette.fg_muted),
            PillTone::Merged => (palette.magenta, palette.bg),
            PillTone::Info => (palette.cyan, palette.bg),
        }
    }

    /// Icon used inside pills and for per-check lines.
    pub fn icon(self) -> &'static str {
        match self {
            PillTone::Success => "✓",
            PillTone::Failure => "✗",
            PillTone::Pending => "●",
            PillTone::Running => "◐",
            PillTone::Neutral => "○",
            PillTone::Merged => "⇌",
            PillTone::Info => "◆",
        }
    }

    /// Foreground colour for non-pill (plain icon) rendering.
    pub fn fg(self, palette: &ColorPalette) -> Color {
        match self {
            PillTone::Success => palette.green,
            PillTone::Failure => palette.red,
            PillTone::Pending => palette.yellow,
            PillTone::Running => palette.blue,
            PillTone::Neutral => palette.fg_dim,
            PillTone::Merged => palette.magenta,
            PillTone::Info => palette.cyan,
        }
    }
}

/// A padded, coloured chip span: ` ✓ label `.
pub fn pill(label: &str, tone: PillTone, palette: &ColorPalette) -> Span<'static> {
    let (bg, fg) = tone.colors(palette);
    Span::styled(
        format!(" {} {} ", tone.icon(), label),
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
    )
}

/// Map a normalised CI check state (uppercase) to a pill tone.
pub fn ci_state_tone(state: &str) -> PillTone {
    match state {
        "SUCCESS" | "COMPLETED" => PillTone::Success,
        "FAILURE" | "ERROR" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => {
            PillTone::Failure
        }
        "PENDING" | "QUEUED" | "WAITING" | "REQUESTED" | "EXPECTED" => PillTone::Pending,
        "IN_PROGRESS" => PillTone::Running,
        _ => PillTone::Neutral, // CANCELLED, SKIPPED, NEUTRAL, STALE, unknown
    }
}

/// Human label for a normalised CI check state.
pub fn ci_state_label(state: &str) -> String {
    match state {
        "SUCCESS" => "Passed".to_string(),
        "FAILURE" => "Failed".to_string(),
        "ERROR" => "Error".to_string(),
        "PENDING" | "QUEUED" | "WAITING" | "REQUESTED" | "EXPECTED" => "Pending".to_string(),
        "IN_PROGRESS" => "Running".to_string(),
        "CANCELLED" => "Cancelled".to_string(),
        "TIMED_OUT" => "Timed out".to_string(),
        "ACTION_REQUIRED" => "Action required".to_string(),
        "SKIPPED" => "Skipped".to_string(),
        "NEUTRAL" => "Neutral".to_string(),
        "STALE" => "Stale".to_string(),
        other => {
            let lower = other.to_lowercase().replace('_', " ");
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

/// Fallback aggregate when the server rollup state is absent.
/// Priority: failure > pending/running > success > neutral.
pub fn aggregate_ci_state(checks: &[crate::preview::CiCheck]) -> String {
    let mut has_pending = false;
    let mut has_success = false;
    for check in checks {
        match ci_state_tone(&check.state) {
            PillTone::Failure => return "FAILURE".to_string(),
            PillTone::Pending | PillTone::Running => has_pending = true,
            PillTone::Success => has_success = true,
            _ => {}
        }
    }
    if has_pending {
        "PENDING".to_string()
    } else if has_success {
        "SUCCESS".to_string()
    } else {
        "NEUTRAL".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::CiCheck;

    fn check(state: &str) -> CiCheck {
        CiCheck {
            name: "c".to_string(),
            state: state.to_string(),
        }
    }

    #[test]
    fn ci_state_tones_cover_known_states() {
        assert_eq!(ci_state_tone("SUCCESS"), PillTone::Success);
        assert_eq!(ci_state_tone("FAILURE"), PillTone::Failure);
        assert_eq!(ci_state_tone("ERROR"), PillTone::Failure);
        assert_eq!(ci_state_tone("TIMED_OUT"), PillTone::Failure);
        assert_eq!(ci_state_tone("PENDING"), PillTone::Pending);
        assert_eq!(ci_state_tone("QUEUED"), PillTone::Pending);
        assert_eq!(ci_state_tone("IN_PROGRESS"), PillTone::Running);
        assert_eq!(ci_state_tone("SKIPPED"), PillTone::Neutral);
        assert_eq!(ci_state_tone("SOMETHING_NEW"), PillTone::Neutral);
    }

    #[test]
    fn aggregate_prefers_failure_then_pending() {
        assert_eq!(
            aggregate_ci_state(&[check("SUCCESS"), check("FAILURE"), check("PENDING")]),
            "FAILURE"
        );
        assert_eq!(
            aggregate_ci_state(&[check("SUCCESS"), check("IN_PROGRESS")]),
            "PENDING"
        );
        assert_eq!(
            aggregate_ci_state(&[check("SUCCESS"), check("SKIPPED")]),
            "SUCCESS"
        );
        assert_eq!(aggregate_ci_state(&[check("SKIPPED")]), "NEUTRAL");
        assert_eq!(aggregate_ci_state(&[]), "NEUTRAL");
    }

    #[test]
    fn labels_are_humanised() {
        assert_eq!(ci_state_label("SUCCESS"), "Passed");
        assert_eq!(ci_state_label("ACTION_REQUIRED"), "Action required");
        assert_eq!(ci_state_label("SOME_NEW_STATE"), "Some new state");
    }

    #[test]
    fn pill_contains_icon_and_label() {
        let palette = ColorPalette::from_name("tokyo_night");
        let span = pill("Passed", PillTone::Success, &palette);
        assert_eq!(span.content.as_ref(), " ✓ Passed ");
    }
}
