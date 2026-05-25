use crate::config::TriageSession;
use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct SessionPickerWidget {
    theme: Theme,
    colors: ColorPalette,
}

impl SessionPickerWidget {
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
        sessions: &[TriageSession],
        selected_index: usize,
        active_session_index: Option<usize>,
    ) {
        let total_items = sessions.len() + 1;
        let max_name_len = sessions
            .iter()
            .map(|session| session.name.len())
            .max()
            .unwrap_or(0)
            .max("Default".len());
        let box_width = (max_name_len + 13).clamp(32, 64) as u16;
        let box_width = box_width.min(area.width.saturating_sub(4));
        let box_height = ((total_items + 4) as u16).min(area.height.saturating_sub(4));
        let centered_area = Rect {
            x: (area.width.saturating_sub(box_width)) / 2,
            y: (area.height.saturating_sub(box_height)) / 2,
            width: box_width,
            height: box_height,
        };

        frame.render_widget(Clear, centered_area);

        let mut content = Vec::new();
        content.push(Line::from(""));
        content.push(self.item_line(
            0,
            "Default",
            selected_index == 0,
            active_session_index.is_none(),
        ));

        for (i, session) in sessions.iter().enumerate() {
            let item_index = i + 1;
            content.push(self.item_line(
                item_index,
                &session.name,
                selected_index == item_index,
                active_session_index == Some(i),
            ));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("0-9", Style::default().fg(self.colors.blue)),
            Span::raw(" select  "),
            Span::styled("j/k", Style::default().fg(self.colors.blue)),
            Span::raw(" move  "),
            Span::styled("Esc", Style::default().fg(self.colors.red)),
            Span::raw(" cancel"),
        ]));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    " Sessions ",
                    Style::default()
                        .fg(self.colors.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(self.colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded);

        frame.render_widget(
            Paragraph::new(content)
                .block(block)
                .alignment(Alignment::Left),
            centered_area,
        );
    }

    fn item_line<'a>(
        &self,
        item_index: usize,
        name: &'a str,
        is_selected: bool,
        is_active: bool,
    ) -> Line<'a> {
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
            Style::default().fg(self.colors.blue)
        };
        let shortcut_label = if item_index <= 9 {
            format!("{}.", item_index)
        } else {
            "  ".to_string()
        };

        Line::from(vec![
            Span::styled(format!(" {} ", indicator), style),
            Span::styled(format!("{} ", shortcut_label), shortcut_style),
            Span::styled(name, style),
            Span::styled(
                format!(" {}", active_mark),
                Style::default().fg(self.colors.cyan),
            ),
        ])
    }
}
