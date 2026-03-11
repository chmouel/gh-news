use crate::ui::theme::Theme;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub struct HelpContent {
    pub lines: Vec<Line<'static>>,
    pub match_count: usize,
    pub total_lines: usize,
}

pub struct HelpLayout {
    pub area: Rect,
    pub inner_height: usize,
    pub inner_width: usize,
}

pub struct HelpWidget {
    theme: Theme,
}

struct HelpLine {
    key: &'static str,
    desc: &'static str,
}

struct HelpSection {
    title: &'static str,
    title_color: Color,
    key_color: Color,
    lines: Vec<HelpLine>,
}

impl HelpWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn layout(area: Rect) -> HelpLayout {
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

        let inner_height = centered_area
            .height
            .saturating_sub(2) // borders
            .saturating_sub(4); // top/bottom padding
        let inner_width = centered_area
            .width
            .saturating_sub(2) // borders
            .saturating_sub(4); // left/right padding

        HelpLayout {
            area: centered_area,
            inner_height: inner_height as usize,
            inner_width: inner_width as usize,
        }
    }

    pub fn content_height(content: &HelpContent, inner_width: usize) -> usize {
        if inner_width == 0 {
            return 0;
        }

        content
            .lines
            .iter()
            .map(|line| {
                let width = line.width().max(1);
                width.div_ceil(inner_width)
            })
            .sum()
    }

    pub fn build_content(filter: Option<&str>) -> HelpContent {
        let colors = crate::ui::theme::TokyoNight::colors();
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());
        let filter_lower = filter.map(|value| value.to_lowercase());

        let sections = vec![
            HelpSection {
                title: "Navigation",
                title_color: colors.cyan,
                key_color: colors.blue,
                lines: vec![
                    HelpLine {
                        key: "↑↓ / j/k",
                        desc: "Navigate list",
                    },
                    HelpLine {
                        key: "Home/End",
                        desc: "Jump to first/last item",
                    },
                    HelpLine {
                        key: "PageUp/Down",
                        desc: "Page navigation",
                    },
                ],
            },
            HelpSection {
                title: "Actions",
                title_color: colors.green,
                key_color: colors.green,
                lines: vec![
                    HelpLine {
                        key: "Enter",
                        desc: "Open in browser (or all selected, toggle repo header)",
                    },
                    HelpLine {
                        key: "o",
                        desc: "Open without marking read",
                    },
                    HelpLine {
                        key: ".",
                        desc: "Toggle read status (or mark selected)",
                    },
                    HelpLine {
                        key: "d",
                        desc: "Archive (done) notification(s)",
                    },
                    HelpLine {
                        key: "!",
                        desc: "Pin/unpin notification",
                    },
                    HelpLine {
                        key: "h",
                        desc: "Collapse current repository",
                    },
                    HelpLine {
                        key: "x",
                        desc: "Run custom action on notification(s)",
                    },
                    HelpLine {
                        key: "Ctrl+A",
                        desc: "Archive selected (or mark all read)",
                    },
                    HelpLine {
                        key: "Ctrl+R",
                        desc: "Refresh notifications",
                    },
                ],
            },
            HelpSection {
                title: "Multi-select",
                title_color: colors.magenta,
                key_color: colors.magenta,
                lines: vec![
                    HelpLine {
                        key: "Space",
                        desc: "Toggle selection (auto-advance)",
                    },
                    HelpLine {
                        key: "Ctrl+Alt+A",
                        desc: "Toggle select all notifications in repo",
                    },
                    HelpLine {
                        key: "Esc",
                        desc: "Clear selection",
                    },
                ],
            },
            HelpSection {
                title: "View & Filter",
                title_color: colors.orange,
                key_color: colors.orange,
                lines: vec![
                    HelpLine {
                        key: "A",
                        desc: "Toggle showing read notifications",
                    },
                    HelpLine {
                        key: "E",
                        desc: "Expunge read notifications",
                    },
                    HelpLine {
                        key: "/",
                        desc: "Filter notifications",
                    },
                    HelpLine {
                        key: "M",
                        desc: "Toggle auto-mark-read (persisted)",
                    },
                ],
            },
            HelpSection {
                title: "Preview",
                title_color: colors.yellow,
                key_color: colors.yellow,
                lines: vec![
                    HelpLine {
                        key: "Tab",
                        desc: "Cycle preview modes (Off → H → V)",
                    },
                    HelpLine {
                        key: "J/K",
                        desc: "Scroll preview (line by line)",
                    },
                    HelpLine {
                        key: "Shift+U/D",
                        desc: "Scroll preview (5 lines)",
                    },
                    HelpLine {
                        key: "Ctrl+U/D",
                        desc: "Scroll preview (page)",
                    },
                    HelpLine {
                        key: "1/2",
                        desc: "Focus pane 1 (list) / 2 (preview)",
                    },
                ],
            },
            HelpSection {
                title: "Exit",
                title_color: colors.red,
                key_color: colors.red,
                lines: vec![HelpLine {
                    key: "Esc / q / Ctrl+C",
                    desc: "Quit application",
                }],
            },
        ];

        fn make_key_line(line: &HelpLine, key_color: Color, fg_muted: Color) -> Line<'static> {
            let key_width = 18;
            let key_text = format!("{:<width$}", line.key, width = key_width);
            Line::from(vec![
                Span::raw("  ".to_string()),
                Span::styled(
                    key_text,
                    Style::default().fg(key_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ".to_string(), Style::default().fg(fg_muted)),
                Span::raw(line.desc.to_string()),
            ])
        }

        fn contains_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
            haystack.to_lowercase().contains(needle_lower)
        }

        let mut lines = Vec::new();
        let mut match_count = 0;
        let mut total_lines = 0;

        for section in sections {
            total_lines += section.lines.len();

            let heading_line = Line::from(vec![Span::styled(
                section.title,
                Style::default()
                    .fg(section.title_color)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )])
            .centered();

            let mut section_lines = Vec::new();
            let mut include_section = true;

            if let Some(ref filter_lower) = filter_lower {
                let heading_matches = contains_case_insensitive(section.title, filter_lower);
                if heading_matches {
                    section_lines = section
                        .lines
                        .iter()
                        .map(|line| make_key_line(line, section.key_color, colors.fg_muted))
                        .collect();
                    match_count += section.lines.len();
                } else {
                    for line in &section.lines {
                        let haystack = format!("{} {}", line.key, line.desc);
                        if contains_case_insensitive(&haystack, filter_lower) {
                            section_lines.push(make_key_line(
                                line,
                                section.key_color,
                                colors.fg_muted,
                            ));
                            match_count += 1;
                        }
                    }
                }

                include_section = heading_matches || !section_lines.is_empty();
            } else {
                section_lines = section
                    .lines
                    .iter()
                    .map(|line| make_key_line(line, section.key_color, colors.fg_muted))
                    .collect();
                match_count = total_lines;
            }

            if include_section {
                lines.push(heading_line);
                lines.push(Line::from(""));
                lines.extend(section_lines);
                lines.push(Line::from(""));
            }
        }

        if filter_lower.is_some() && match_count == 0 {
            lines.clear();
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                format!("No matches for \"{}\"", filter.unwrap_or_default()),
                Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
        }

        HelpContent {
            lines,
            match_count,
            total_lines,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        layout: HelpLayout,
        content: &HelpContent,
        scroll: usize,
    ) {
        let colors = crate::ui::theme::TokyoNight::colors();
        let scroll = scroll.min(u16::MAX as usize) as u16;

        let footer = Line::from(vec![
            Span::styled(" Press ", Style::default().fg(colors.fg_muted)),
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
            Span::styled(" to close ", Style::default().fg(colors.fg_muted)),
            Span::styled("•", Style::default().fg(colors.fg_muted)),
            Span::styled(
                " j/k",
                Style::default()
                    .fg(colors.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to scroll ", Style::default().fg(colors.fg_muted)),
        ]);

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
            .title_bottom(footer)
            .border_style(Style::default().fg(colors.cyan))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(ratatui::widgets::Padding::new(2, 2, 2, 2));

        let paragraph = Paragraph::new(content.lines.clone())
            .block(block)
            .style(self.theme.help_text)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .alignment(Alignment::Left)
            .scroll((scroll, 0));

        frame.render_widget(paragraph, layout.area);
    }
}
