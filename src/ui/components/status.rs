use crate::config::Config;
use crate::state::AppState;
use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::time::Instant;

/// Refresh state passed to the status widget for countdown display.
pub struct RefreshState {
    pub last_refresh: Instant,
    pub is_refreshing: bool,
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
        let interval_text = if config.auto_refresh_interval > 0 {
            if refresh.is_refreshing {
                "🔄 refreshing…".to_string()
            } else {
                let elapsed = refresh.last_refresh.elapsed().as_secs();
                let interval = config.auto_refresh_interval;
                let remaining = interval.saturating_sub(elapsed);
                if remaining >= 60 {
                    format!("🔄 {}m{:02}s", remaining / 60, remaining % 60)
                } else {
                    format!("🔄 {}s", remaining)
                }
            }
        } else {
            String::new()
        };

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
