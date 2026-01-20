use crate::markdown::MarkdownRenderer;
use crate::preview::PreviewData;
use crate::state::AppState;
use crate::ui::theme::{Theme, TokyoNight};
use ratatui::{
    prelude::*,
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph, Scrollbar, ScrollbarState,
    },
};

pub struct PreviewWidget {
    theme: Theme,
    scrollbar_state: ScrollbarState,
    cached_lines: Vec<Line<'static>>,
    cached_body: Option<String>,
}

impl PreviewWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            scrollbar_state: ScrollbarState::default(),
            cached_lines: Vec::new(),
            cached_body: None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        let colors = TokyoNight::colors();

        // Main block with border
        let block = Block::default()
            .borders(Borders::ALL)
            .title(
                Line::from(" 󰨞 Details ").left_aligned().style(
                    Style::default()
                        .fg(self.theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .title(
                Title::from(
                    Line::from(" ? Help q Quit ").style(Style::default().fg(colors.fg_dim)),
                )
                .alignment(Alignment::Right)
                .position(Position::Bottom),
            )
            .border_style(self.theme.border)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if let Some(preview_data) = &state.preview_content {
            self.render_preview_data(frame, inner_area, preview_data, state, &colors);
        } else {
            // No preview available
            let empty_text = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "📄 No preview available",
                    Style::default().fg(colors.fg_dim),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Select a notification to view its content",
                    Style::default().fg(colors.fg_muted),
                )]),
            ];

            let paragraph = Paragraph::new(empty_text)
                .style(self.theme.preview)
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, inner_area);
        }
    }

    fn render_preview_data(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        preview_data: &PreviewData,
        state: &AppState,
        colors: &TokyoNight,
    ) {
        // Render all content (header, separator, description) as scrollable
        self.render_scrollable_content(frame, area, preview_data, state, colors);
    }

    fn get_header_lines(
        &self,
        preview_data: &PreviewData,
        colors: &TokyoNight,
    ) -> Vec<Line<'static>> {
        match preview_data {
            PreviewData::PullRequest {
                number,
                title,
                state,
                author,
                comments,
                mergeable,
                ..
            } => {
                let state_color = match state.as_str() {
                    "open" => colors.green,
                    "closed" => colors.red,
                    _ => colors.fg_dim,
                };

                vec![
                    Line::from(vec![
                        Span::styled("PR #", Style::default().fg(colors.blue)),
                        Span::styled(
                            number.clone(),
                            Style::default()
                                .fg(colors.blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" - "),
                        Span::styled(
                            title.clone(),
                            Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            format!("[{}]", state),
                            Style::default()
                                .fg(state_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Author: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(author.clone(), Style::default().fg(colors.cyan)),
                        Span::raw(" | "),
                        Span::styled("Comments: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(comments.to_string(), Style::default().fg(colors.yellow)),
                        Span::raw(" | "),
                        Span::styled("Mergeable: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(
                            mergeable.clone(),
                            Style::default().fg(if mergeable == "Yes" {
                                colors.green
                            } else {
                                colors.red
                            }),
                        ),
                    ]),
                ]
            }
            PreviewData::Issue {
                number,
                title,
                state,
                author,
                comments,
                ..
            } => {
                let state_color = match state.as_str() {
                    "open" => colors.green,
                    "closed" => colors.red,
                    _ => colors.fg_dim,
                };

                vec![
                    Line::from(vec![
                        Span::styled("Issue #", Style::default().fg(colors.magenta)),
                        Span::styled(
                            number.clone(),
                            Style::default()
                                .fg(colors.magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" - "),
                        Span::styled(
                            title.clone(),
                            Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            format!("[{}]", state),
                            Style::default()
                                .fg(state_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Author: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(author.clone(), Style::default().fg(colors.cyan)),
                        Span::raw(" | "),
                        Span::styled("Comments: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(comments.to_string(), Style::default().fg(colors.yellow)),
                    ]),
                ]
            }
            PreviewData::Commit { sha, author, .. } => {
                vec![
                    Line::from(vec![
                        Span::styled("Commit ", Style::default().fg(colors.yellow)),
                        Span::styled(
                            sha.chars().take(12).collect::<String>(),
                            Style::default()
                                .fg(colors.yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Author: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(author.clone(), Style::default().fg(colors.cyan)),
                    ]),
                ]
            }
            PreviewData::Release {
                tag,
                name,
                published_at,
                prerelease,
                ..
            } => {
                vec![
                    Line::from(vec![
                        Span::styled("Release ", Style::default().fg(colors.orange)),
                        Span::styled(
                            tag.clone(),
                            Style::default()
                                .fg(colors.orange)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" - "),
                        Span::styled(
                            name.clone(),
                            Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Published: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(published_at.clone(), Style::default().fg(colors.fg_muted)),
                        Span::raw(" | "),
                        Span::styled("Pre-release: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(
                            if *prerelease { "Yes" } else { "No" },
                            Style::default().fg(if *prerelease {
                                colors.yellow
                            } else {
                                colors.green
                            }),
                        ),
                    ]),
                ]
            }
            PreviewData::SecurityAlert {
                severity,
                vulnerability_count,
                affected_packages,
                ..
            } => {
                let severity_color = match severity.to_lowercase().as_str() {
                    "critical" | "high" => colors.red,
                    "medium" => colors.yellow,
                    "low" => colors.orange,
                    _ => colors.fg_dim,
                };

                vec![
                    Line::from(vec![
                        Span::styled(
                            "⚠️  ",
                            Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "Security Alert",
                            Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Severity: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(
                            severity.clone(),
                            Style::default()
                                .fg(severity_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" | "),
                        Span::styled("Vulnerabilities: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(
                            vulnerability_count.to_string(),
                            Style::default()
                                .fg(colors.yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Affected Packages: ", Style::default().fg(colors.fg_dim)),
                        Span::styled(
                            if affected_packages.is_empty() {
                                "None specified".to_string()
                            } else {
                                affected_packages.join(", ")
                            },
                            Style::default().fg(colors.cyan),
                        ),
                    ]),
                ]
            }
            PreviewData::Generic { title, .. } => {
                vec![Line::from(vec![Span::styled(
                    title.clone(),
                    Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
                )])]
            }
        }
    }

    fn get_separator_line(&self, width: u16, colors: &TokyoNight) -> Line<'static> {
        Line::from(vec![Span::styled(
            "─".repeat(width as usize),
            Style::default().fg(colors.bg_dark),
        )])
    }

    fn render_scrollable_content(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        preview_data: &PreviewData,
        state: &AppState,
        colors: &TokyoNight,
    ) {
        // Get header lines (3 lines)
        let header_lines = self.get_header_lines(preview_data, colors);

        // Get separator line (1 line)
        let separator_line = self.get_separator_line(area.width, colors);

        // Get body content
        let body = match preview_data {
            PreviewData::PullRequest { body, .. } => body,
            PreviewData::Issue { body, .. } => body,
            PreviewData::Commit { body, .. } => body,
            PreviewData::Release { body, .. } => body,
            PreviewData::SecurityAlert { body, .. } => body,
            PreviewData::Generic { body, .. } => body,
        };

        // Only re-render markdown when body content changes
        if self.cached_body.as_ref() != Some(body) {
            let body_lines = MarkdownRenderer::render_simple(body);

            let mut all_lines = Vec::new();
            all_lines.extend(header_lines);
            all_lines.push(separator_line);
            all_lines.extend(body_lines);

            self.cached_body = Some(body.clone());
            self.cached_lines = all_lines;
        }

        let content_height = self.cached_lines.len();
        let visible_height = area.height as usize;

        self.scrollbar_state = self
            .scrollbar_state
            .content_length(content_height)
            .viewport_content_length(visible_height)
            .position(state.preview_scroll);

        // Get visible lines based on scroll
        let start = state
            .preview_scroll
            .min(self.cached_lines.len().saturating_sub(1));
        let end = (start + visible_height).min(self.cached_lines.len());
        let visible_lines: Vec<Line> = if self.cached_lines.is_empty() {
            vec![Line::from(vec![Span::styled(
                "No description",
                Style::default().fg(colors.fg_dim),
            )])]
        } else {
            self.cached_lines[start..end].to_vec()
        };

        let paragraph = Paragraph::new(visible_lines)
            .style(self.theme.preview)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(paragraph, area);

        // Render scrollbar
        if content_height > visible_height {
            let scrollbar = Scrollbar::default()
                .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
                .thumb_symbol("█")
                .track_symbol(Some("│"));

            frame.render_stateful_widget(scrollbar, area, &mut self.scrollbar_state);
        }
    }
}
