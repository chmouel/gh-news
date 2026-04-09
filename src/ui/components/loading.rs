use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

pub struct LoadingWidget {
    theme: Theme,
}

impl LoadingWidget {
    pub fn new(palette: &ColorPalette) -> Self {
        Self {
            theme: Theme::from_palette(palette),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        message: &str,
        progress: Option<(usize, usize)>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let tick = now.as_millis() as u64;
        let pulse_on = (tick / 450).is_multiple_of(2);
        let dot_count = (tick / 250) % 4;
        let dots = ".".repeat(dot_count as usize);

        let card_width = area.width.min(72);
        let card_height = area.height.min(11);
        let card_area = centred_rect(area, card_width, card_height);

        let border_style = if pulse_on {
            Style::default()
                .fg(self.theme.highlight_fg)
                .add_modifier(Modifier::DIM)
        } else {
            self.theme.border
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Loading ", self.theme.title))
            .border_style(border_style)
            .border_type(ratatui::widgets::BorderType::Rounded);

        frame.render_widget(block.clone(), card_area);

        let inner = block.inner(card_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(inner);

        let title_style = Style::default()
            .fg(self.theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        let message_line = if let Some((current, total)) = progress {
            format!("{message} ({current}/{total}){dots}")
        } else {
            format!("{message}{dots}")
        };
        let hint_style = Style::default()
            .fg(self.theme.highlight_fg)
            .add_modifier(Modifier::DIM);

        let text = vec![
            Line::from(Span::styled("gh-news", title_style)),
            Line::from(""),
            Line::from(Span::styled(message_line, self.theme.preview)),
            Line::from(""),
            Line::from(Span::styled("Press q to quit", hint_style)),
        ];

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[0]);

        let progress_percent = if let Some((current, total)) = progress {
            if total > 0 {
                ((current * 100) / total) as u16
            } else {
                0
            }
        } else {
            ((tick / 90) % 100) as u16
        };
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .bg(self.theme.highlight_bg),
            )
            .percent(progress_percent);

        frame.render_widget(gauge, chunks[1]);
    }
}

fn centred_rect(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);

    horizontal[1]
}
