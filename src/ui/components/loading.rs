use crate::ui::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

pub struct LoadingWidget {
    theme: Theme,
}

impl LoadingWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, message: &str) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(area);

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                Span::styled(message, self.theme.help_text),
            ]),
            Line::from(""),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Loading")
                    .border_style(self.theme.border)
                    .border_type(ratatui::widgets::BorderType::Rounded),
            )
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, chunks[0]);

        // Animated progress bar
        let progress = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_secs() % 100) as u16)
            .unwrap_or(0);

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(progress);

        frame.render_widget(gauge, chunks[1]);
    }
}
