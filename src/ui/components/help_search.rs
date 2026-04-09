use crate::ui::theme::ColorPalette;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct HelpSearchWidget {
    colors: ColorPalette,
}

impl HelpSearchWidget {
    pub fn new(palette: &ColorPalette) -> Self {
        Self {
            colors: palette.clone(),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        query: &str,
        match_count: usize,
        total_count: usize,
        active: bool,
    ) {
        let colors = &self.colors;

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

        frame.render_widget(Clear, filter_area);

        let mut spans = vec![Span::styled(query, Style::default().fg(colors.fg))];
        if active {
            spans.push(Span::styled(
                "_",
                Style::default()
                    .fg(colors.cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ));
        }
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("({}/{})", match_count, total_count),
            Style::default().fg(colors.fg_dim),
        ));

        let content = Line::from(spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(
                    " / ",
                    Style::default()
                        .fg(colors.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Help search ", Style::default().fg(colors.fg)),
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
