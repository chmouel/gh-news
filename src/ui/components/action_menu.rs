use crate::builtin_actions::CombinedAction;
use crate::ui::theme::{Theme, TokyoNight};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ActionMenuWidget {
    theme: Theme,
}

impl ActionMenuWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        actions: &[CombinedAction],
        selected_index: usize,
        notification_count: usize,
    ) {
        let colors = TokyoNight::colors();

        // Calculate box dimensions based on content
        let max_name_len = actions.iter().map(|a| a.name().len()).max().unwrap_or(10);
        let box_width = (max_name_len + 10).clamp(30, 60) as u16;
        let box_width = box_width.min(area.width.saturating_sub(4));

        // Height: 2 for borders + 1 for header + actions + 2 for keybindings
        let content_height = actions.len() as u16 + 4;
        let box_height = content_height.min(area.height.saturating_sub(4));

        let box_x = (area.width.saturating_sub(box_width)) / 2;
        let box_y = (area.height.saturating_sub(box_height)) / 2;

        let centered_area = Rect {
            x: box_x,
            y: box_y,
            width: box_width,
            height: box_height,
        };

        // Clear the background area
        frame.render_widget(Clear, centered_area);

        // Build content lines
        let mut content = Vec::new();
        content.push(Line::from(""));

        for (i, action) in actions.iter().enumerate() {
            let is_selected = i == selected_index;
            let indicator = if is_selected { ">" } else { " " };

            let style = if is_selected {
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.theme.title
            };

            content.push(Line::from(vec![
                Span::styled(format!(" {} ", indicator), style),
                Span::styled(action.name(), style),
            ]));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("j/k", Style::default().fg(colors.blue)),
            Span::raw(" select  "),
            Span::styled("Enter", Style::default().fg(colors.green)),
            Span::raw(" run  "),
            Span::styled("Esc", Style::default().fg(colors.red)),
            Span::raw(" cancel"),
        ]));

        // Build title
        let title_text = if notification_count > 1 {
            format!(" Run on {} Notifications ", notification_count)
        } else {
            " Actions ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    title_text,
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
