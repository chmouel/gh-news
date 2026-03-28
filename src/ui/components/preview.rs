use crate::markdown::MarkdownRenderer;
use crate::preview::{PreviewData, PreviewHeaderKind, PreviewView};
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
    cached_signature: Option<String>,
}

impl PreviewWidget {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            scrollbar_state: ScrollbarState::default(),
            cached_lines: Vec::new(),
            cached_signature: None,
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
            let empty_text =
                if state.filtered_notifications.is_empty() && !state.notifications.is_empty() {
                    vec![
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "🔍 Filtering active",
                            Style::default().fg(colors.fg_dim),
                        )]),
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "No notifications match your filter",
                            Style::default().fg(colors.fg_muted),
                        )]),
                    ]
                } else {
                    vec![
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
                    ]
                };

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
        preview_view: &PreviewView,
        colors: &TokyoNight,
    ) -> Vec<Line<'static>> {
        preview_view
            .header
            .iter()
            .map(|line| {
                let spans: Vec<Span> = line
                    .parts
                    .iter()
                    .map(|part| {
                        let style = match part.kind {
                            PreviewHeaderKind::Title => {
                                Style::default().fg(colors.fg).add_modifier(Modifier::BOLD)
                            }
                            PreviewHeaderKind::Label => Style::default().fg(colors.fg_dim),
                            PreviewHeaderKind::Author => Style::default().fg(colors.cyan),
                            PreviewHeaderKind::Count => Style::default()
                                .fg(colors.yellow)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::Date => Style::default().fg(colors.fg_muted),
                            PreviewHeaderKind::PackageList => Style::default().fg(colors.cyan),
                            PreviewHeaderKind::Dim => Style::default().fg(colors.fg_dim),
                            PreviewHeaderKind::AccentPullRequest => Style::default()
                                .fg(colors.blue)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::AccentIssue => Style::default()
                                .fg(colors.magenta)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::AccentCommit => Style::default()
                                .fg(colors.yellow)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::AccentRelease => Style::default()
                                .fg(colors.orange)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::AccentDiscussion => Style::default()
                                .fg(colors.cyan)
                                .add_modifier(Modifier::BOLD),
                            PreviewHeaderKind::Warning => {
                                Style::default().fg(colors.red).add_modifier(Modifier::BOLD)
                            }
                            PreviewHeaderKind::Status => {
                                let lower = part.text.to_lowercase();
                                let status_color = match lower.as_str() {
                                    "open" | "yes" => colors.green,
                                    "merged" => colors.magenta,
                                    "closed" | "no" => colors.red,
                                    "critical" | "high" => colors.red,
                                    "medium" => colors.yellow,
                                    "low" => colors.orange,
                                    "unknown" => colors.fg_dim,
                                    _ => colors.fg,
                                };
                                Style::default()
                                    .fg(status_color)
                                    .add_modifier(Modifier::BOLD)
                            }
                        };
                        Span::styled(part.text.clone(), style)
                    })
                    .collect();
                Line::from(spans)
            })
            .collect()
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
        let preview_view = PreviewView::from(preview_data);

        // Get header lines (3 lines)
        let header_lines = self.get_header_lines(&preview_view, colors);

        // Get separator line (1 line)
        let separator_line = self.get_separator_line(area.width, colors);

        // Get body content
        let body = preview_view.body.as_str();
        let header_signature = preview_view
            .header
            .iter()
            .map(|line| line.text())
            .collect::<Vec<_>>()
            .join("\n");
        let signature = format!("{header_signature}\n{body}");

        // Only re-render markdown when content changes
        if self.cached_signature.as_ref() != Some(&signature) {
            let body_lines = MarkdownRenderer::render_simple(body);

            let mut all_lines = Vec::new();
            all_lines.extend(header_lines);
            all_lines.push(separator_line);
            all_lines.extend(body_lines);

            self.cached_signature = Some(signature);
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
