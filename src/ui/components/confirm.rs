use crate::state::MarkAllOption;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ConfirmWidget;

impl ConfirmWidget {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        selected: MarkAllOption,
        count: usize,
        is_filtered: bool,
    ) {
        let colors = crate::ui::theme::TokyoNight::colors();

        // Calculate centered box dimensions (compact dialog)
        let box_width = 50.min(area.width.saturating_sub(4));
        let box_height = 10.min(area.height.saturating_sub(4));
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

        // Selection indicators
        let (archive_indicator, read_indicator) = match selected {
            MarkAllOption::MarkReadAndArchive => ("[*]", "[ ]"),
            MarkAllOption::MarkReadOnly => ("[ ]", "[*]"),
        };

        let content = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    archive_indicator,
                    Style::default().fg(if selected == MarkAllOption::MarkReadAndArchive {
                        colors.green
                    } else {
                        colors.fg_muted
                    }),
                ),
                Span::raw(" Mark as read and archive"),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    read_indicator,
                    Style::default().fg(if selected == MarkAllOption::MarkReadOnly {
                        colors.green
                    } else {
                        colors.fg_muted
                    }),
                ),
                Span::styled(" Just mark as read ", Style::default()),
                Span::styled("(default)", Style::default().fg(colors.fg_muted)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("j/k", Style::default().fg(colors.blue)),
                Span::raw(" select  "),
                Span::styled("Enter", Style::default().fg(colors.green)),
                Span::raw(" confirm  "),
                Span::styled("Esc", Style::default().fg(colors.red)),
                Span::raw(" cancel"),
            ]),
        ];

        // Build title based on count and filter state
        let title_text = if is_filtered {
            format!(" Mark {} Filtered Notifications ", count)
        } else {
            format!(" Mark {} Notifications ", count)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    title_text,
                    Style::default()
                        .fg(colors.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(colors.yellow))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}
