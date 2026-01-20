use crate::config::Config;
use crate::state::AppState;
use crate::ui::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub struct StatusWidget {
    theme: Theme,
}

impl StatusWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState, config: &Config) {
        let colors = crate::ui::theme::TokyoNight::colors();

        let interval_text = if config.auto_refresh_interval > 0 {
            if config.auto_refresh_interval >= 60 {
                format!("🔄 {}min", config.auto_refresh_interval / 60)
            } else {
                format!("🔄 {}s", config.auto_refresh_interval)
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
            Span::styled("❓ ", Style::default().fg(colors.yellow)),
            Span::styled("help", self.theme.status_bar),
            Span::raw(" · "),
            Span::styled("ESC ", Style::default().fg(colors.red)),
            Span::styled("quit", self.theme.status_bar),
        ];

        let mut status_line = Line::from(status_parts);

        // Show status message if present (e.g., after marking all as read)
        if let Some(ref msg) = state.status_message {
            status_line.spans.push(Span::raw(" · "));
            status_line.spans.push(Span::styled(
                format!("✓ {}", msg),
                Style::default().fg(colors.green),
            ));
        }

        if !interval_text.is_empty() {
            status_line.spans.push(Span::raw(" · "));
            status_line
                .spans
                .push(Span::styled(interval_text, self.theme.status_bar));
        }

        // Show active filter indicator when not in search mode
        if let Some(ref pattern) = state.filter_pattern {
            status_line.spans.push(Span::raw(" · "));
            status_line
                .spans
                .push(Span::styled("🔍 ", Style::default().fg(colors.cyan)));
            status_line.spans.push(Span::styled(
                format!("\"{}\"", pattern),
                Style::default().fg(colors.cyan),
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
                Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
            ));
        }

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
