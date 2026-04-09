use crate::builtin_actions::{shortcut_for_index, CombinedAction};
use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ActionMenuWidget {
    theme: Theme,
    colors: ColorPalette,
}

impl ActionMenuWidget {
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
        actions: &[CombinedAction],
        selected_index: usize,
        notification_count: usize,
    ) {
        let max_name_len = actions.iter().map(|a| a.name().len()).max().unwrap_or(10);
        let box_width = (max_name_len + 13).clamp(30, 60) as u16;
        let box_width = box_width.min(area.width.saturating_sub(4));

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

        frame.render_widget(Clear, centered_area);

        let mut content = Vec::new();
        content.push(Line::from(""));

        for (i, action) in actions.iter().enumerate() {
            let is_selected = i == selected_index;
            let indicator = if is_selected { ">" } else { " " };
            let shortcut_label = shortcut_for_index(i)
                .map(|c| format!("{}.", c))
                .unwrap_or_else(|| "  ".to_string());

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
                Style::default().fg(self.colors.blue)
            };

            content.push(Line::from(vec![
                Span::styled(format!(" {} ", indicator), style),
                Span::styled(format!("{} ", shortcut_label), shortcut_style),
                Span::styled(action.name(), style),
            ]));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("1-9/a-z", Style::default().fg(self.colors.blue)),
            Span::raw(" run  "),
            Span::styled("j/k", Style::default().fg(self.colors.blue)),
            Span::raw(" select  "),
            Span::styled("Esc", Style::default().fg(self.colors.red)),
            Span::raw(" cancel"),
        ]));

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
                        .fg(self.colors.magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(self.colors.magenta))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}
