use crate::state::AppState;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct FilterWidget;

impl FilterWidget {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &AppState) {
        let colors = crate::ui::theme::TokyoNight::colors();

        // Small box in top-right corner
        let box_width = 50.min(area.width.saturating_sub(4));
        let box_height = 3;
        let box_x = area.width.saturating_sub(box_width).saturating_sub(2);
        let box_y = 1;

        let filter_area = Rect {
            x: box_x,
            y: box_y,
            width: box_width,
            height: box_height,
        };

        // Clear the background
        frame.render_widget(Clear, filter_area);

        // Build content with query and cursor
        let content = Line::from(vec![
            Span::styled(&state.search_query, Style::default().fg(colors.fg)),
            Span::styled(
                "_",
                Style::default()
                    .fg(colors.cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "({}/{})",
                    state.filtered_notifications.len(),
                    state.notifications.len()
                ),
                Style::default().fg(colors.fg_dim),
            ),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(
                    " / ",
                    Style::default()
                        .fg(colors.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Filter ", Style::default().fg(colors.fg)),
                Span::styled("Enter", Style::default().fg(colors.green)),
                Span::styled(" keep ", Style::default().fg(colors.fg_dim)),
                Span::styled("Esc", Style::default().fg(colors.red)),
                Span::styled(" clear ", Style::default().fg(colors.fg_dim)),
            ])
            .border_style(Style::default().fg(colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, filter_area);
    }
}
