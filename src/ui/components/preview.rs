use crate::emoji::expand_shortcodes;
use crate::markdown::MarkdownRenderer;
use crate::preview::{
    LatestComment, PreviewData, PreviewHeaderKind, PreviewView, TimelineEntry, TimelineKind,
};
use crate::state::AppState;
use crate::ui::components::pills::{
    aggregate_ci_state, ci_state_label, ci_state_tone, pill, PillTone,
};
use crate::ui::theme::{ColorPalette, Theme};
use ratatui::{
    prelude::*,
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph, Scrollbar, ScrollbarState,
    },
};

pub struct PreviewWidget {
    theme: Theme,
    colors: ColorPalette,
    scrollbar_state: ScrollbarState,
    cached_lines: Vec<Line<'static>>,
    cached_signature: Option<String>,
    /// Wrapped line count of `cached_lines` for a given width, so
    /// `Paragraph::line_count` is not recomputed every frame.
    cached_height: Option<(u16, usize)>,
    expanded_ci_key: Option<String>,
}

impl PreviewWidget {
    pub fn new(palette: &ColorPalette) -> Self {
        Self {
            theme: Theme::from_palette(palette),
            colors: palette.clone(),
            scrollbar_state: ScrollbarState::default(),
            cached_lines: Vec::new(),
            cached_signature: None,
            cached_height: None,
            expanded_ci_key: None,
        }
    }

    pub fn toggle_ci_checks(&mut self, state: &AppState) {
        let has_checks = matches!(
            state.preview_content,
            Some(PreviewData::PullRequest { ref ci_checks, .. }) if !ci_checks.is_empty()
        );
        let Some(key) = has_checks
            .then(|| state.selected_notification().map(|n| n.preview_cache_key()))
            .flatten()
        else {
            return;
        };

        if self.expanded_ci_key.as_deref() == Some(&key) {
            self.expanded_ci_key = None;
        } else {
            self.expanded_ci_key = Some(key);
        }
        self.cached_signature = None;
        self.cached_height = None;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
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
                    Line::from(" ? Help q Quit ").style(Style::default().fg(self.colors.fg_dim)),
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
            self.render_preview_data(frame, inner_area, preview_data, state);
        } else {
            // No preview available
            let empty_text =
                if state.filtered_notifications.is_empty() && !state.notifications.is_empty() {
                    vec![
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "🔍 Filtering active",
                            Style::default().fg(self.colors.fg_dim),
                        )]),
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "No notifications match your filter",
                            Style::default().fg(self.colors.fg_muted),
                        )]),
                    ]
                } else {
                    vec![
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "📄 No preview available",
                            Style::default().fg(self.colors.fg_dim),
                        )]),
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "Select a notification to view its content",
                            Style::default().fg(self.colors.fg_muted),
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
    ) {
        self.render_scrollable_content(frame, area, preview_data, state);
    }

    fn get_header_lines(&self, preview_view: &PreviewView) -> Vec<Line<'static>> {
        let colors = &self.colors;
        preview_view
            .header
            .iter()
            .map(|line| {
                let spans: Vec<Span> =
                    line.parts
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
                                PreviewHeaderKind::AccentWorkflowRun => Style::default()
                                    .fg(colors.orange)
                                    .add_modifier(Modifier::BOLD),
                                PreviewHeaderKind::AccentActivityEvent => Style::default()
                                    .fg(colors.cyan)
                                    .add_modifier(Modifier::BOLD),
                                PreviewHeaderKind::Warning => {
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD)
                                }
                                PreviewHeaderKind::Tag => Style::default()
                                    .fg(colors.cyan)
                                    .add_modifier(Modifier::ITALIC),
                                PreviewHeaderKind::Status => {
                                    let lower = part.text.to_lowercase();
                                    let status_color = match lower.as_str() {
                                        "open" | "yes" | "approved" | "success" => colors.green,
                                        "merged" => colors.magenta,
                                        "closed" | "no" | "changes_requested" | "failure"
                                        | "error" => colors.red,
                                        "draft" | "pending" | "expected" | "review_required" => {
                                            colors.yellow
                                        }
                                        "critical" | "high" => colors.red,
                                        "medium" => colors.yellow,
                                        "low" => colors.orange,
                                        "cancelled" | "skipped" | "timed_out" => colors.yellow,
                                        "completed" | "in_progress" | "queued" | "waiting" => {
                                            colors.blue
                                        }
                                        "none" | "unknown" => colors.fg_dim,
                                        _ => colors.fg,
                                    };
                                    Style::default()
                                        .fg(status_color)
                                        .add_modifier(Modifier::BOLD)
                                }
                            };
                            let text = if part.kind == PreviewHeaderKind::Title {
                                expand_shortcodes(&part.text)
                            } else {
                                part.text.clone()
                            };
                            Span::styled(text, style)
                        })
                        .collect();
                Line::from(spans)
            })
            .collect()
    }

    fn get_separator_line(&self, width: u16) -> Line<'static> {
        Line::from(vec![Span::styled(
            "─".repeat(width as usize),
            Style::default().fg(self.colors.bg_dark),
        )])
    }

    fn render_scrollable_content(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        preview_data: &PreviewData,
        state: &AppState,
    ) {
        let is_pull_request = matches!(preview_data, PreviewData::PullRequest { .. });

        if is_pull_request {
            let selected_key = state
                .selected_notification()
                .map(|notification| notification.preview_cache_key());
            let ci_expanded = selected_key.as_ref() == self.expanded_ci_key.as_ref();
            let cutoff = state
                .selected_notification()
                .and_then(|n| state.effective_read_cutoff(n));
            let signature = format!(
                "pr|{}|{:?}|{}|{}",
                state.preview_content_version, cutoff, area.width, ci_expanded
            );
            if self.cached_signature.as_ref() != Some(&signature) {
                self.cached_lines = self.build_pr_lines(preview_data, cutoff, ci_expanded);
                self.cached_signature = Some(signature);
                self.cached_height = None;
            }
        } else {
            let preview_view = PreviewView::from(preview_data);
            let header_lines = self.get_header_lines(&preview_view);
            let separator_line = self.get_separator_line(area.width);

            let body = preview_view.body.as_str();
            let header_signature = preview_view
                .header
                .iter()
                .map(|line| line.text())
                .collect::<Vec<_>>()
                .join("\n");
            let signature = format!("{header_signature}\n{body}");

            if self.cached_signature.as_ref() != Some(&signature) {
                let body_lines = MarkdownRenderer::render_simple(body, &self.colors);

                let mut all_lines = Vec::new();
                all_lines.extend(header_lines);
                all_lines.push(separator_line);
                all_lines.extend(body_lines);

                self.cached_signature = Some(signature);
                self.cached_lines = all_lines;
                self.cached_height = None;
            }
        }

        let visible_height = area.height as usize;
        let lines: Vec<Line> = if self.cached_lines.is_empty() {
            vec![Line::from(vec![Span::styled(
                "No description",
                Style::default().fg(self.colors.fg_dim),
            )])]
        } else {
            self.cached_lines.clone()
        };

        let paragraph = Paragraph::new(lines)
            .style(self.theme.preview)
            .wrap(ratatui::widgets::Wrap { trim: false });

        // Wrapped visual rows, so long comment lines scroll correctly.
        // Recomputed only when the cached content or the pane width changes.
        let content_height = match self.cached_height {
            Some((width, height)) if width == area.width => height,
            _ => {
                let height = paragraph.line_count(area.width);
                self.cached_height = Some((area.width, height));
                height
            }
        };
        let max_scroll = content_height.saturating_sub(visible_height);
        let offset = state.preview_scroll.min(max_scroll);

        self.scrollbar_state = self
            .scrollbar_state
            .content_length(content_height)
            .viewport_content_length(visible_height)
            .position(offset);

        let paragraph = paragraph.scroll((offset.min(u16::MAX as usize) as u16, 0));
        frame.render_widget(paragraph, area);

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

    fn section_heading(&self, text: &str, extra: Vec<Span<'static>>) -> Line<'static> {
        let mut spans = vec![
            Span::styled(
                "▌ ",
                Style::default()
                    .fg(self.colors.blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                text.to_string(),
                Style::default()
                    .fg(self.colors.fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if !extra.is_empty() {
            spans.push(Span::raw("  "));
            spans.extend(extra);
        }
        Line::from(spans)
    }

    /// Build the redesigned pull-request preview: readiness pills, the
    /// new-activity feed, then CI checks.
    fn build_pr_lines(
        &self,
        preview_data: &PreviewData,
        read_cutoff: Option<chrono::DateTime<chrono::Utc>>,
        ci_expanded: bool,
    ) -> Vec<Line<'static>> {
        let PreviewData::PullRequest {
            number,
            title,
            state: pr_state,
            author,
            mergeable,
            labels,
            review_decision,
            is_draft,
            ci_status,
            additions,
            deletions,
            changed_files,
            body,
            latest_comment,
            ci_checks,
            ci_total_count,
            merge_state_status,
            head_ref_oid,
            base_ref,
            head_ref,
            timeline,
            timeline_total_count,
            ..
        } = preview_data
        else {
            return Vec::new();
        };

        let colors = &self.colors;
        let mut lines: Vec<Line<'static>> = Vec::new();

        // ── Title ────────────────────────────────────────────────────────
        let state_text = if *is_draft && pr_state == "open" {
            "draft"
        } else {
            pr_state.as_str()
        };
        let state_tone = match state_text {
            "open" => PillTone::Success,
            "merged" => PillTone::Merged,
            "closed" => PillTone::Failure,
            _ => PillTone::Neutral,
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("PR #{number} "),
                Style::default()
                    .fg(colors.blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                expand_shortcodes(title),
                Style::default().fg(colors.fg).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            pill(state_text, state_tone, colors),
        ]));

        // ── Meta line: author · branches · diffstat ─────────────────────
        let mut meta = vec![Span::styled(
            format!("@{author}"),
            Style::default().fg(colors.cyan),
        )];
        if !base_ref.is_empty() && !head_ref.is_empty() {
            meta.push(Span::styled(" · ", Style::default().fg(colors.fg_dim)));
            meta.push(Span::styled(
                format!("{base_ref} ← {head_ref}"),
                Style::default().fg(colors.fg_muted),
            ));
        }
        meta.push(Span::styled(" · ", Style::default().fg(colors.fg_dim)));
        meta.push(Span::styled(
            format!("+{additions}"),
            Style::default().fg(colors.green),
        ));
        meta.push(Span::raw(" "));
        meta.push(Span::styled(
            format!("−{deletions}"),
            Style::default().fg(colors.red),
        ));
        meta.push(Span::styled(
            format!(" · {changed_files} files"),
            Style::default().fg(colors.fg_muted),
        ));
        lines.push(Line::from(meta));

        if !labels.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                labels
                    .iter()
                    .map(|l| format!("({l})"))
                    .collect::<Vec<_>>()
                    .join(" "),
                Style::default()
                    .fg(colors.cyan)
                    .add_modifier(Modifier::ITALIC),
            )]));
        }
        lines.push(Line::from(""));

        // ── Merge readiness pills ────────────────────────────────────────
        let mut pills_row: Vec<Span<'static>> = Vec::new();
        let mergeable_tone = match mergeable.as_str() {
            "Yes" => PillTone::Success,
            "No" => PillTone::Failure,
            _ => PillTone::Neutral,
        };
        pills_row.push(pill(
            match mergeable.as_str() {
                "Yes" => "Mergeable",
                "No" => "Conflicts",
                _ => "Mergeable?",
            },
            mergeable_tone,
            colors,
        ));
        pills_row.push(Span::raw(" "));

        let (review_label, review_tone) = match review_decision.as_str() {
            "APPROVED" => ("Approved", PillTone::Success),
            "CHANGES_REQUESTED" => ("Changes requested", PillTone::Failure),
            "REVIEW_REQUIRED" => ("Review required", PillTone::Pending),
            _ => ("No review", PillTone::Neutral),
        };
        pills_row.push(pill(review_label, review_tone, colors));
        pills_row.push(Span::raw(" "));

        let effective_ci = if ci_status == "UNKNOWN" && !ci_checks.is_empty() {
            aggregate_ci_state(ci_checks)
        } else {
            ci_status.clone()
        };
        if effective_ci != "UNKNOWN" {
            pills_row.push(pill(
                &format!("CI {}", ci_state_label(&effective_ci)),
                ci_state_tone(&effective_ci),
                colors,
            ));
            pills_row.push(Span::raw(" "));
        }

        if !merge_state_status.is_empty() {
            let (ms_label, ms_tone) = match merge_state_status.as_str() {
                "CLEAN" => ("Ready to merge", PillTone::Success),
                "BLOCKED" => ("Blocked", PillTone::Failure),
                "BEHIND" => ("Behind base", PillTone::Pending),
                "DIRTY" => ("Conflicts", PillTone::Failure),
                "UNSTABLE" => ("Unstable", PillTone::Pending),
                "DRAFT" => ("Draft", PillTone::Neutral),
                "HAS_HOOKS" => ("Has hooks", PillTone::Info),
                other => (other, PillTone::Neutral),
            };
            pills_row.push(pill(ms_label, ms_tone, colors));
        }
        lines.push(Line::from(pills_row));

        let merge_ready = pr_state == "open" && !*is_draft && !head_ref_oid.is_empty();
        if merge_ready {
            lines.push(Line::from(vec![Span::styled(
                "  press m to merge",
                Style::default()
                    .fg(colors.fg_dim)
                    .add_modifier(Modifier::ITALIC),
            )]));
        }
        lines.push(Line::from(""));

        // ── Description ──────────────────────────────────────────────────
        if !body.trim().is_empty() {
            lines.push(self.section_heading("Description", Vec::new()));
            lines.push(Line::from(""));
            lines.extend(MarkdownRenderer::render_simple(body, colors));
            lines.push(Line::from(""));
        }

        // ── New activity since last read ─────────────────────────────────
        let activity = merge_pr_activity(timeline, latest_comment.as_ref());
        let new_entries: Vec<&TimelineEntry> = match read_cutoff {
            Some(cutoff) => activity
                .iter()
                .filter(|entry| activity_is_new(entry, cutoff))
                .collect(),
            // Never read: avoid dumping the whole thread.
            None => activity.iter().rev().take(1).rev().collect(),
        };

        if !new_entries.is_empty() {
            lines.push(self.section_heading(
                &format!("New since last read ({})", new_entries.len()),
                Vec::new(),
            ));
            let shown_earliest = activity
                .iter()
                .position(|e| std::ptr::eq(e, *new_entries.first().unwrap()))
                .unwrap_or(0);
            if *timeline_total_count > timeline.len() as u64 && shown_earliest == 0 {
                lines.push(Line::from(vec![Span::styled(
                    "  … earlier activity not shown",
                    Style::default().fg(colors.fg_dim),
                )]));
            }
            for entry in &new_entries {
                lines.push(Line::from(""));
                lines.extend(self.timeline_entry_lines(entry));
            }
            lines.push(Line::from(""));
        }

        // ── CI checks ────────────────────────────────────────────────────
        if !ci_checks.is_empty() {
            let mut counts = [0_u64; 5];
            for check in ci_checks {
                let index = match ci_state_tone(&check.state) {
                    PillTone::Success => 0,
                    PillTone::Failure => 1,
                    PillTone::Running => 2,
                    PillTone::Pending => 3,
                    _ => 4,
                };
                counts[index] += 1;
            }

            let mut summary = Vec::new();
            for (count, label, tone) in [
                (counts[0], "passed", PillTone::Success),
                (counts[1], "failed", PillTone::Failure),
                (counts[2], "running", PillTone::Running),
                (counts[3], "pending", PillTone::Pending),
                (counts[4], "other", PillTone::Neutral),
            ] {
                if count > 0 {
                    summary.push(pill(&format!("{count} {label}"), tone, colors));
                    summary.push(Span::raw(" "));
                }
            }

            lines.push(self.section_heading(
                if ci_expanded {
                    "CI Checks ▾"
                } else {
                    "CI Checks ▸"
                },
                summary,
            ));
            lines.push(Line::from(Span::styled(
                if ci_expanded {
                    "  c collapse checks"
                } else {
                    "  c expand checks"
                },
                Style::default()
                    .fg(colors.fg_dim)
                    .add_modifier(Modifier::ITALIC),
            )));

            if ci_expanded {
                for check in ci_checks {
                    let tone = ci_state_tone(&check.state);
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            tone.icon().to_string(),
                            Style::default()
                                .fg(tone.fg(colors))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(check.name.clone(), Style::default().fg(colors.fg)),
                    ]));
                }
            }
            if ci_expanded && *ci_total_count > ci_checks.len() as u64 {
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "  … {} more checks not shown",
                        ci_total_count - ci_checks.len() as u64
                    ),
                    Style::default().fg(colors.fg_dim),
                )]));
            }
        }

        lines
    }

    /// Attribution line plus markdown-rendered body for one timeline entry.
    fn timeline_entry_lines(&self, entry: &TimelineEntry) -> Vec<Line<'static>> {
        let colors = &self.colors;
        let (verb, tone, icon) = match entry.kind {
            TimelineKind::Comment => ("commented", PillTone::Info, "💬"),
            TimelineKind::Approved => ("approved", PillTone::Success, "✅"),
            TimelineKind::ChangesRequested => ("requested changes", PillTone::Failure, "✋"),
            TimelineKind::Commented => ("reviewed", PillTone::Info, "👀"),
            TimelineKind::Dismissed => ("dismissed a review", PillTone::Neutral, "🚫"),
        };
        let mut lines = vec![Line::from(vec![
            Span::raw(format!("{icon} ")),
            Span::styled(
                format!("@{}", entry.author),
                Style::default()
                    .fg(colors.cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                verb.to_string(),
                Style::default()
                    .fg(tone.fg(colors))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" · {}", format_timestamp(&entry.timestamp)),
                Style::default().fg(colors.fg_dim),
            ),
        ])];
        if !entry.body.is_empty() {
            lines.extend(MarkdownRenderer::render_simple(&entry.body, colors));
        }
        lines
    }
}

/// Add the notification's triggering comment to the GraphQL activity feed.
/// The REST URL can identify activity that is absent from the selected
/// GraphQL timeline item types, so merge and deduplicate before filtering.
fn merge_pr_activity(
    timeline: &[TimelineEntry],
    latest_comment: Option<&LatestComment>,
) -> Vec<TimelineEntry> {
    let mut activity = timeline.to_vec();
    if let Some(comment) = latest_comment {
        let entry = TimelineEntry {
            author: comment.author.clone(),
            kind: TimelineKind::Comment,
            body: comment.body.clone(),
            timestamp: comment.created_at.clone(),
        };
        let duplicate = activity.iter().any(|existing| {
            existing.kind == entry.kind
                && existing.author == entry.author
                && existing.body == entry.body
                && existing.timestamp == entry.timestamp
        });
        if !duplicate {
            activity.push(entry);
        }
    }
    activity.sort_by(|left, right| {
        parse_timestamp(&left.timestamp)
            .cmp(&parse_timestamp(&right.timestamp))
            .then_with(|| left.timestamp.cmp(&right.timestamp))
    });
    activity
}

fn parse_timestamp(timestamp: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&chrono::Utc))
}

fn activity_is_new(entry: &TimelineEntry, cutoff: chrono::DateTime<chrono::Utc>) -> bool {
    parse_timestamp(&entry.timestamp)
        .map(|timestamp| timestamp > cutoff)
        .unwrap_or(false)
}

/// Format an RFC 3339 timestamp as a local, human-readable date.
fn format_timestamp(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| timestamp.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(author: &str, body: &str, timestamp: &str) -> TimelineEntry {
        TimelineEntry {
            author: author.to_string(),
            kind: TimelineKind::Comment,
            body: body.to_string(),
            timestamp: timestamp.to_string(),
        }
    }

    #[test]
    fn triggering_comment_is_merged_in_timestamp_order() {
        let timeline = vec![
            entry("alice", "first", "2026-07-01T10:00:00Z"),
            entry("carol", "third", "2026-07-01T12:00:00Z"),
        ];
        let latest = LatestComment {
            author: "bob".to_string(),
            body: "second".to_string(),
            created_at: "2026-07-01T11:00:00Z".to_string(),
        };

        let activity = merge_pr_activity(&timeline, Some(&latest));

        assert_eq!(activity.len(), 3);
        assert_eq!(activity[1].author, "bob");
    }

    #[test]
    fn triggering_comment_is_deduplicated() {
        let timeline = vec![entry("alice", "same", "2026-07-01T10:00:00Z")];
        let latest = LatestComment {
            author: "alice".to_string(),
            body: "same".to_string(),
            created_at: "2026-07-01T10:00:00Z".to_string(),
        };

        assert_eq!(merge_pr_activity(&timeline, Some(&latest)).len(), 1);
    }

    #[test]
    fn triggering_comment_respects_read_cutoff() {
        let latest = LatestComment {
            author: "alice".to_string(),
            body: "already read".to_string(),
            created_at: "2026-07-01T10:00:00Z".to_string(),
        };
        let cutoff = parse_timestamp("2026-07-01T11:00:00Z").unwrap();
        let activity = merge_pr_activity(&[], Some(&latest));

        assert!(!activity_is_new(&activity[0], cutoff));
    }
}
