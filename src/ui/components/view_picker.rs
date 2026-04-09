use crate::config::View;
use crate::ui::theme::{Theme, TokyoNight};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ViewPickerWidget {
    theme: Theme,
}

impl ViewPickerWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        views: &[View],
        selected_index: usize,
        active_view_index: Option<usize>,
    ) {
        let colors = TokyoNight::colors();

        // Items: [0] Default, [1..] named views
        let total_items = views.len() + 1;

        let max_name_len = views
            .iter()
            .map(|v| v.name.len())
            .max()
            .unwrap_or(0)
            .max("Default".len());
        let box_width = (max_name_len + 13).clamp(30, 60) as u16;
        let box_width = box_width.min(area.width.saturating_sub(4));

        // 2 borders + 1 blank + items + 1 blank + 1 footer
        let content_height = (total_items + 4) as u16;
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

        // Item 0: Default (no view)
        {
            let is_selected = selected_index == 0;
            let is_active = active_view_index.is_none();
            let indicator = if is_selected { ">" } else { " " };
            let active_mark = if is_active { "*" } else { " " };

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
                Span::styled("0. ", shortcut_style),
                Span::styled("Default", style),
                Span::styled(
                    format!(" {}", active_mark),
                    Style::default().fg(colors.cyan),
                ),
            ]));
        }

        // Named views
        for (i, view) in views.iter().enumerate() {
            let item_index = i + 1;
            let is_selected = selected_index == item_index;
            let is_active = active_view_index == Some(i);
            let indicator = if is_selected { ">" } else { " " };
            let active_mark = if is_active { "*" } else { " " };

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

            let shortcut_label = if item_index <= 9 {
                format!("{}.", item_index)
            } else {
                "  ".to_string()
            };

            content.push(Line::from(vec![
                Span::styled(format!(" {} ", indicator), style),
                Span::styled(format!("{} ", shortcut_label), shortcut_style),
                Span::styled(view.name.clone(), style),
                Span::styled(
                    format!(" {}", active_mark),
                    Style::default().fg(colors.cyan),
                ),
            ]));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("0-9", Style::default().fg(colors.blue)),
            Span::raw(" select  "),
            Span::styled("j/k", Style::default().fg(colors.blue)),
            Span::raw(" move  "),
            Span::styled("Esc", Style::default().fg(colors.red)),
            Span::raw(" cancel"),
        ]));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    " Views ",
                    Style::default()
                        .fg(colors.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}
