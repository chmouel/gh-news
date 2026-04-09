use crate::ui::theme::{Theme, TokyoNight};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub const URL_MENU_ITEMS: &[&str] = &["Open in browser", "Copy URL (OSC 52)", "Print URL"];

pub struct UrlMenuWidget {
    theme: Theme,
}

impl UrlMenuWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, selected_index: usize) {
        let colors = TokyoNight::colors();

        let box_width = 32u16.min(area.width.saturating_sub(4));
        let box_height = 8u16.min(area.height.saturating_sub(4));

        let box_x = (area.width.saturating_sub(box_width)) / 2;
        let box_y = (area.height.saturating_sub(box_height)) / 2;

        let centered_area = Rect {
            x: box_x,
            y: box_y,
            width: box_width,
            height: box_height,
        };

        frame.render_widget(Clear, centered_area);

        let mut content = Vec::new();
        content.push(Line::from(""));

        for (i, label) in URL_MENU_ITEMS.iter().enumerate() {
            let is_selected = i == selected_index;
            let indicator = if is_selected { ">" } else { " " };
            let shortcut = format!("{}.", i + 1);

            let style = if is_selected {
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.theme.title
            };

            let shortcut_style = if is_selected {
                style
            } else {
                Style::default().fg(colors.blue)
            };

            content.push(Line::from(vec![
                Span::styled(format!(" {} ", indicator), style),
                Span::styled(format!("{} ", shortcut), shortcut_style),
                Span::styled(*label, style),
            ]));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("1-3", Style::default().fg(colors.blue)),
            Span::raw(" run  "),
            Span::styled("j/k", Style::default().fg(colors.blue)),
            Span::raw(" select  "),
            Span::styled("Esc", Style::default().fg(colors.red)),
            Span::raw(" cancel"),
        ]));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    " Open URL ",
                    Style::default()
                        .fg(colors.magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(colors.magenta))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}
