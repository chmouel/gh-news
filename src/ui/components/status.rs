use crate::config::Config;
use crate::state::AppState;
use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::time::Instant;

const REFRESH_SPINNER_FRAMES: &[char] = &['|', '/', '-', '\\'];

/// Refresh state passed to the status widget for countdown and spinner display.
pub struct RefreshState {
    pub last_refresh: Instant,
    pub is_refreshing: bool,
    pub spinner_frame_index: usize,
}

pub struct StatusWidget {
    theme: Theme,
    colors: ColorPalette,
}

impl StatusWidget {
    pub fn new(palette: &ColorPalette) -> Self {
        Self {
            theme: Theme::from_palette(palette),
            colors: palette.clone(),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        config: &Config,
        auto_mark_read: bool,
        refresh: &RefreshState,
    ) {
        let interval_text = refresh_indicator_text(config.auto_refresh_interval, refresh);

        let unread_count = state
            .filtered_notifications
            .iter()
            .filter(|&&idx| {
                state
                    .notifications
                    .get(idx)
                    .map(|n| n.is_unread())
                    .unwrap_or(false)
            })
            .count();

        let status_parts = vec![
            Span::styled("❓ ", Style::default().fg(self.colors.yellow)),
            Span::styled("help", self.theme.status_bar),
            Span::raw(" · "),
            Span::styled("ESC ", Style::default().fg(self.colors.red)),
            Span::styled("quit", self.theme.status_bar),
        ];

        let mut status_line = Line::from(status_parts);

        if let Some(ref msg) = state.status_message {
            status_line.spans.push(Span::raw(" · "));
            status_line.spans.push(Span::styled(
                format!("✓ {}", msg),
                Style::default().fg(self.colors.green),
            ));
        }

        if !interval_text.is_empty() {
            status_line.spans.push(Span::raw(" · "));
            let style = if refresh.is_refreshing {
                Style::default()
                    .fg(self.colors.cyan)
                    .add_modifier(Modifier::ITALIC)
            } else {
                self.theme.status_bar
            };
            status_line.spans.push(Span::styled(interval_text, style));
        }

        if let Some(view_name) = state.active_view_name() {
            status_line.spans.push(Span::raw(" · "));
            status_line
                .spans
                .push(Span::styled("👁 ", Style::default().fg(self.colors.cyan)));
            status_line.spans.push(Span::styled(
                view_name.to_string(),
                Style::default()
                    .fg(self.colors.cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if let Some(ref pattern) = state.filter_pattern {
            status_line.spans.push(Span::raw(" · "));
            status_line
                .spans
                .push(Span::styled("🔍 ", Style::default().fg(self.colors.cyan)));
            status_line.spans.push(Span::styled(
                format!("\"{}\"", pattern),
                Style::default().fg(self.colors.cyan),
            ));
        }

        status_line.spans.push(Span::raw(" · "));
        status_line.spans.push(Span::styled(
            format!("📬 {} notifications", state.visible_count()),
            self.theme.status_bar,
        ));

        if unread_count > 0 {
            status_line.spans.push(Span::raw(" · "));
            status_line.spans.push(Span::styled(
                format!("🔴 {} unread", unread_count),
                Style::default()
                    .fg(self.colors.red)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if let Some(rate_limit) = state.rate_limit {
            if let (Some(remaining), Some(limit)) = (rate_limit.remaining, rate_limit.limit) {
                status_line.spans.push(Span::raw(" · "));
                let style = if remaining < 100 {
                    Style::default()
                        .fg(self.colors.red)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.colors.fg_muted)
                };
                status_line
                    .spans
                    .push(Span::styled(format!("API {remaining}/{limit}"), style));
            }
        }

        let mode_label = if auto_mark_read { "M:read" } else { "M:off" };
        status_line.spans.push(Span::raw(" · "));
        status_line.spans.push(Span::styled(
            mode_label,
            if auto_mark_read {
                Style::default().fg(self.colors.green)
            } else {
                Style::default().fg(self.colors.fg_muted)
            },
        ));

        let paragraph = Paragraph::new(status_line)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(self.theme.border)
                    .padding(ratatui::widgets::Padding::new(0, 1, 0, 1)),
            )
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }
}

fn refresh_indicator_text(auto_refresh_interval: u64, refresh: &RefreshState) -> String {
    if refresh.is_refreshing {
        let frame = REFRESH_SPINNER_FRAMES
            .get(refresh.spinner_frame_index % REFRESH_SPINNER_FRAMES.len())
            .copied()
            .unwrap_or('|');
        return format!("{frame} refreshing...");
    }

    if auto_refresh_interval == 0 {
        return String::new();
    }

    let elapsed = refresh.last_refresh.elapsed().as_secs();
    let remaining = auto_refresh_interval.saturating_sub(elapsed);
    if remaining >= 60 {
        format!("🔄 {}m{:02}s", remaining / 60, remaining % 60)
    } else {
        format!("🔄 {}s", remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::{refresh_indicator_text, RefreshState};
    use std::time::{Duration, Instant};

    #[test]
    fn refresh_indicator_text_uses_spinner_frame_when_refreshing() {
        let refresh = RefreshState {
            last_refresh: Instant::now(),
            is_refreshing: true,
            spinner_frame_index: 1,
        };

        assert_eq!(refresh_indicator_text(120, &refresh), "/ refreshing...");
    }

    #[test]
    fn refresh_indicator_text_cycles_spinner_frames() {
        let refresh = RefreshState {
            last_refresh: Instant::now(),
            is_refreshing: true,
            spinner_frame_index: 7,
        };

        assert_eq!(refresh_indicator_text(120, &refresh), "\\ refreshing...");
    }

    #[test]
    fn refresh_indicator_text_shows_countdown_when_idle() {
        let refresh = RefreshState {
            last_refresh: Instant::now() - Duration::from_secs(30),
            is_refreshing: false,
            spinner_frame_index: 0,
        };

        assert_eq!(refresh_indicator_text(120, &refresh), "🔄 1m30s");
    }

    #[test]
    fn refresh_indicator_text_shows_spinner_even_without_auto_refresh_interval() {
        let refresh = RefreshState {
            last_refresh: Instant::now(),
            is_refreshing: true,
            spinner_frame_index: 2,
        };

        assert_eq!(refresh_indicator_text(0, &refresh), "- refreshing...");
    }

    #[test]
    fn refresh_indicator_text_is_empty_when_idle_and_auto_refresh_disabled() {
        let refresh = RefreshState {
            last_refresh: Instant::now(),
            is_refreshing: false,
            spinner_frame_index: 0,
        };

        assert_eq!(refresh_indicator_text(0, &refresh), "");
    }
}
