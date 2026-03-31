use crate::state::CommandOutputData;
use crate::ui::theme::TokyoNight;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarState, Wrap},
};

pub struct CommandOutputWidget {
    scrollbar_state: ScrollbarState,
}

impl CommandOutputWidget {
    pub fn new() -> Self {
        Self {
            scrollbar_state: ScrollbarState::default(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, data: &CommandOutputData) {
        let colors = TokyoNight::colors();

        // Large centered popup: ~80% of terminal dimensions
        let box_width = (area.width * 4 / 5)
            .max(40)
            .min(area.width.saturating_sub(4));
        let box_height = (area.height * 4 / 5)
            .max(10)
            .min(area.height.saturating_sub(4));
        let box_x = (area.width.saturating_sub(box_width)) / 2;
        let box_y = (area.height.saturating_sub(box_height)) / 2;

        let popup_area = Rect {
            x: box_x,
            y: box_y,
            width: box_width,
            height: box_height,
        };

        frame.render_widget(Clear, popup_area);

        // Inner area excluding border (1 cell) and footer line (1 cell)
        let inner_width = popup_area.width.saturating_sub(2) as usize;
        let visible_lines = popup_area.height.saturating_sub(3) as usize; // border top/bottom + footer

        // Wrap content into lines respecting inner width
        let content_lines: Vec<Line> = data
            .content
            .lines()
            .flat_map(|line| {
                if line.is_empty() {
                    vec![Line::from("")]
                } else if inner_width == 0 {
                    vec![Line::from(line.to_string())]
                } else {
                    // Split long lines at inner_width boundaries
                    line.as_bytes()
                        .chunks(inner_width)
                        .map(|chunk| Line::from(String::from_utf8_lossy(chunk).into_owned()))
                        .collect()
                }
            })
            .collect();

        let content_height = content_lines.len();
        let scroll = data
            .scroll
            .min(content_height.saturating_sub(visible_lines));

        // Visible slice
        let visible: Vec<Line> = content_lines
            .into_iter()
            .skip(scroll)
            .take(visible_lines)
            .collect();

        // Update scrollbar state
        self.scrollbar_state = self
            .scrollbar_state
            .content_length(content_height)
            .viewport_content_length(visible_lines)
            .position(scroll);

        // Footer line
        let footer = Line::from(vec![
            Span::styled("j/k", Style::default().fg(colors.blue)),
            Span::raw(" scroll  "),
            Span::styled("PgUp/PgDn", Style::default().fg(colors.blue)),
            Span::raw(" page  "),
            Span::styled("q/Esc", Style::default().fg(colors.red)),
            Span::raw(" close"),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::raw(" "),
                Span::styled(
                    format!(" {} ", data.title),
                    Style::default()
                        .fg(colors.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ])
            .title_alignment(Alignment::Center)
            .title_bottom(footer)
            .border_style(Style::default().fg(colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(visible)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, popup_area);

        // Scrollbar
        if content_height > visible_lines {
            let scrollbar = Scrollbar::default()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .thumb_symbol("█")
                .track_symbol(Some("│"));

            frame.render_stateful_widget(scrollbar, popup_area, &mut self.scrollbar_state);
        }
    }
}
