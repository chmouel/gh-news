use crate::ui::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub struct HelpWidget {
    theme: Theme,
}

impl HelpWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let colors = crate::ui::theme::TokyoNight::colors();

        // Calculate centered box dimensions
        let box_width = 68.min(area.width.saturating_sub(4));
        let box_height = 40.min(area.height.saturating_sub(4));
        let box_x = (area.width.saturating_sub(box_width)) / 2;
        let box_y = (area.height.saturating_sub(box_height)) / 2;

        let centered_area = Rect {
            x: box_x,
            y: box_y,
            width: box_width,
            height: box_height,
        };

        // Helper function to create key binding lines
        fn make_key_line(
            key: &str,
            desc: &str,
            key_color: Color,
            fg_muted: Color,
        ) -> Line<'static> {
            let key_width = 18;
            let key_text = format!("{:<width$}", key, width = key_width);
            Line::from(vec![
                Span::raw("  ".to_string()),
                Span::styled(
                    key_text,
                    Style::default().fg(key_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ".to_string(), Style::default().fg(fg_muted)),
                Span::raw(desc.to_string()),
            ])
        }

        // Create styled sections with centered alignment
        let help_content = vec![
            // Header with decorative line
            // Navigation Section
            Line::from(vec![Span::styled(
                "Navigation",
                Style::default()
                    .fg(colors.cyan)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line("↑↓ / j/k", "Navigate list", colors.blue, colors.fg_muted),
            make_key_line(
                "Home/End",
                "Jump to first/last item",
                colors.blue,
                colors.fg_muted,
            ),
            make_key_line(
                "PageUp/Down",
                "Page navigation",
                colors.blue,
                colors.fg_muted,
            ),
            Line::from(""),
            // Actions Section
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(colors.green)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line(
                "Enter",
                "Open in browser (or all selected)",
                colors.green,
                colors.fg_muted,
            ),
            make_key_line(
                "o",
                "Open without marking read",
                colors.green,
                colors.fg_muted,
            ),
            make_key_line(
                ".",
                "Toggle read status (or mark selected)",
                colors.green,
                colors.fg_muted,
            ),
            make_key_line("!", "Pin/unpin notification", colors.green, colors.fg_muted),
            make_key_line(
                "h",
                "Collapse current repository",
                colors.green,
                colors.fg_muted,
            ),
            make_key_line(
                "Ctrl+A",
                "Archive selected (or mark all read)",
                colors.green,
                colors.fg_muted,
            ),
            make_key_line(
                "Ctrl+R",
                "Refresh notifications",
                colors.green,
                colors.fg_muted,
            ),
            Line::from(""),
            // Multi-select Section
            Line::from(vec![Span::styled(
                "Multi-select",
                Style::default()
                    .fg(colors.magenta)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line(
                "Space",
                "Toggle selection (auto-advance)",
                colors.magenta,
                colors.fg_muted,
            ),
            make_key_line("Esc", "Clear selection", colors.magenta, colors.fg_muted),
            Line::from(""),
            // View Section
            Line::from(vec![Span::styled(
                "View & Filter",
                Style::default()
                    .fg(colors.orange)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line(
                "A",
                "Toggle showing read notifications",
                colors.orange,
                colors.fg_muted,
            ),
            make_key_line("/", "Filter notifications", colors.orange, colors.fg_muted),
            make_key_line(
                "M",
                "Toggle auto-mark-read on scroll",
                colors.orange,
                colors.fg_muted,
            ),
            Line::from(""),
            // Preview Section
            Line::from(vec![Span::styled(
                "Preview",
                Style::default()
                    .fg(colors.yellow)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line(
                "Tab",
                "Cycle preview modes (Off → H → V)",
                colors.yellow,
                colors.fg_muted,
            ),
            make_key_line(
                "J/K",
                "Scroll preview (line by line)",
                colors.yellow,
                colors.fg_muted,
            ),
            make_key_line(
                "Shift+U/D",
                "Scroll preview (5 lines)",
                colors.yellow,
                colors.fg_muted,
            ),
            make_key_line(
                "Ctrl+U/D",
                "Scroll preview (page)",
                colors.yellow,
                colors.fg_muted,
            ),
            make_key_line(
                "1/2",
                "Focus pane 1 (list) / 2 (preview)",
                colors.yellow,
                colors.fg_muted,
            ),
            Line::from(""),
            // Exit Section
            Line::from(vec![Span::styled(
                "Exit",
                Style::default()
                    .fg(colors.red)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered(),
            Line::from(""),
            make_key_line(
                "Esc / q / Ctrl+C",
                "Quit application",
                colors.red,
                colors.fg_muted,
            ),
            Line::from(""),
            // Footer with decorative line
            Line::from(vec![
                Span::styled("  Press ", Style::default().fg(colors.fg_muted)),
                Span::styled(
                    "?",
                    Style::default()
                        .fg(colors.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" or ", Style::default().fg(colors.fg_muted)),
                Span::styled(
                    "q",
                    Style::default()
                        .fg(colors.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to close", Style::default().fg(colors.fg_muted)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    "💡 Help",
                    Style::default()
                        .fg(colors.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(ratatui::widgets::Padding::new(2, 2, 2, 2));

        let paragraph = Paragraph::new(help_content)
            .block(block)
            .style(self.theme.help_text)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, centered_area);
    }
}
