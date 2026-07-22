use crate::state::{ConfirmAction, MarkAllOption};
use crate::ui::theme::ColorPalette;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ConfirmWidget {
    colors: ColorPalette,
}

impl ConfirmWidget {
    pub fn new(palette: &ColorPalette) -> Self {
        Self {
            colors: palette.clone(),
        }
    }

    fn title_text(action: &ConfirmAction, count: usize, is_filtered: bool) -> String {
        match action {
            ConfirmAction::ArchiveSelected { count, .. } => {
                format!(" Archive {} Selected Notifications ", count)
            }
            ConfirmAction::MarkAllRead { .. } => {
                if is_filtered {
                    format!(" Mark {} Filtered Notifications ", count)
                } else {
                    format!(" Mark {} Notifications ", count)
                }
            }
            ConfirmAction::MergePullRequest { number, .. } => {
                format!(" Merge Pull Request #{} ", number)
            }
        }
    }

    /// Dedicated Enter/Esc-only dialog for merging a pull request.
    fn render_merge(
        &self,
        frame: &mut Frame,
        centered_area: Rect,
        repo: &str,
        number: u64,
        title: &str,
        method: crate::config::MergeMethod,
    ) {
        let colors = &self.colors;
        let max_title = centered_area.width.saturating_sub(6) as usize;
        let mut display_title = title.to_string();
        if display_title.chars().count() > max_title {
            display_title = display_title
                .chars()
                .take(max_title.saturating_sub(1))
                .collect();
            display_title.push('…');
        }

        let content = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    display_title,
                    Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{repo}#{number}"), Style::default().fg(colors.cyan)),
                Span::styled(" · method: ", Style::default().fg(colors.fg_dim)),
                Span::styled(
                    method.api_value(),
                    Style::default()
                        .fg(colors.magenta)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Enter", Style::default().fg(colors.green)),
                Span::raw(" merge  "),
                Span::styled("Esc", Style::default().fg(colors.red)),
                Span::raw(" cancel"),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    format!(" Merge Pull Request #{} ", number),
                    Style::default()
                        .fg(colors.green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(colors.green))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        action: &ConfirmAction,
        count: usize,
        is_filtered: bool,
    ) {
        let colors = &self.colors;

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

        frame.render_widget(Clear, centered_area);

        if let ConfirmAction::MergePullRequest {
            repo,
            number,
            title,
            method,
            ..
        } = action
        {
            self.render_merge(frame, centered_area, repo, *number, title, *method);
            return;
        }

        let selected = match action {
            ConfirmAction::MarkAllRead { selected } => *selected,
            ConfirmAction::ArchiveSelected { option, .. } => *option,
            ConfirmAction::MergePullRequest { .. } => unreachable!(),
        };

        let (archive_indicator, read_indicator) = match selected {
            MarkAllOption::MarkReadAndArchive => ("[*]", "[ ]"),
            MarkAllOption::MarkReadOnly => ("[ ]", "[*]"),
        };

        let is_selected_action = matches!(action, ConfirmAction::ArchiveSelected { .. });

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
                if !is_selected_action {
                    Span::styled("(default)", Style::default().fg(colors.fg_muted))
                } else {
                    Span::raw("")
                },
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

        let title_text = Self::title_text(action, count, is_filtered);

        let border_color = if is_selected_action {
            colors.magenta
        } else {
            colors.yellow
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    title_text,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(border_color))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_mentions_filtered_notifications_for_mark_all() {
        let title = ConfirmWidget::title_text(
            &ConfirmAction::MarkAllRead {
                selected: MarkAllOption::MarkReadOnly,
            },
            23,
            true,
        );
        assert_eq!(title, " Mark 23 Filtered Notifications ");
    }

    #[test]
    fn title_mentions_selected_notifications_for_manual_selection() {
        let title = ConfirmWidget::title_text(
            &ConfirmAction::ArchiveSelected {
                count: 4,
                option: MarkAllOption::MarkReadAndArchive,
            },
            4,
            true,
        );
        assert_eq!(title, " Archive 4 Selected Notifications ");
    }
}
