use crate::models::{NotificationReason, NotificationType};
use crate::state::AppState;
use crate::ui::theme::{Theme, TokyoNight};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub struct ListWidget {
    state: ListState,
    theme: Theme,
}

impl ListWidget {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            state,
            theme: Theme::default(),
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        app_state: &AppState,
        config: &crate::config::Config,
    ) {
        if app_state.filtered_notifications.is_empty() {
            let empty_text = if app_state.notifications.is_empty() {
                "✨ All caught up! No notifications."
            } else {
                "🔍 No notifications match your filters"
            };

            let count = app_state.visible_count();
            let repo_count = app_state
                .tree_items
                .iter()
                .filter(|item| matches!(item, crate::state::TreeItem::RepositoryHeader(_)))
                .count();

            let colors = TokyoNight::colors();
            let (empty_title, empty_title_fg, empty_border) =
                if let Some(view_name) = app_state.active_view_name() {
                    (
                        format!(" 󰎟 {view_name} "),
                        colors.cyan,
                        Style::default().fg(colors.cyan),
                    )
                } else {
                    (
                        " 󰨞1 Notifications ".to_string(),
                        self.theme.highlight_fg,
                        self.theme.border,
                    )
                };

            let paragraph = Paragraph::new(empty_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(
                            Line::from(empty_title).left_aligned().style(
                                Style::default()
                                    .fg(empty_title_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        )
                        .title(
                            Line::from(format!(
                                " 󰂞 {} Notifications 󰉋 {} Repositories ",
                                count, repo_count
                            ))
                            .right_aligned()
                            .style(
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        )
                        .border_style(empty_border)
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                )
                .style(Style::default().fg(Color::Green))
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = app_state
            .tree_items
            .iter()
            .enumerate()
            .map(|(display_idx, item)| {
                let is_selected = display_idx == app_state.selected_index;

                match item {
                    crate::state::TreeItem::PinnedHeader => {
                        let colors = TokyoNight::colors();
                        let mut line = vec![];

                        line.push(Span::styled(
                            "󰐃 ",
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.red)
                            },
                        ));

                        line.push(Span::styled(
                            "Pinned",
                            if is_selected {
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(colors.red).add_modifier(Modifier::BOLD)
                            },
                        ));

                        ListItem::new(Line::from(line))
                    }
                    crate::state::TreeItem::OrgHeader(org_info) => {
                        let colors = TokyoNight::colors();
                        let mut line = vec![];

                        let is_org_expanded = app_state
                            .expanded_orgs
                            .get(&org_info.login)
                            .copied()
                            .unwrap_or(true);

                        line.push(Span::styled(
                            "󰊻 ",
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.magenta)
                            },
                        ));

                        let indicator = if is_org_expanded { "▾ " } else { "▸ " };
                        line.push(Span::styled(
                            indicator,
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.yellow)
                            },
                        ));

                        line.push(Span::styled(
                            format!("{} ", org_info.login),
                            if is_selected {
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                                    .fg(colors.magenta)
                                    .add_modifier(Modifier::BOLD)
                            },
                        ));

                        let count_style = if is_selected {
                            Style::default()
                                .fg(self.theme.highlight_fg)
                                .bg(self.theme.highlight_bg)
                        } else {
                            Style::default().fg(colors.fg).bg(colors.bg_dark)
                        };
                        line.push(Span::styled(
                            format!(" ({}) ", org_info.notification_count),
                            count_style.add_modifier(Modifier::BOLD),
                        ));

                        ListItem::new(Line::from(line))
                    }
                    crate::state::TreeItem::RepositoryHeader(repo_info) => {
                        let mut line = vec![];

                        let colors = TokyoNight::colors();
                        line.push(Span::styled(
                            "",
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.cyan)
                            },
                        ));

                        let indicator = " ";
                        line.push(Span::styled(
                            indicator,
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.yellow)
                            },
                        ));

                        line.push(Span::styled(
                            format!("{} ", repo_info.display_name),
                            if is_selected {
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                self.theme.repo_owner
                            },
                        ));

                        let count_style = if is_selected {
                            Style::default()
                                .fg(self.theme.highlight_fg)
                                .bg(self.theme.highlight_bg)
                        } else {
                            Style::default().fg(colors.fg).bg(colors.bg_dark)
                        };
                        line.push(Span::styled(
                            format!(" ({}) ", repo_info.notification_count),
                            count_style.add_modifier(Modifier::BOLD),
                        ));

                        ListItem::new(Line::from(line))
                    }
                    crate::state::TreeItem::Notification(notif_idx) => {
                        if let Some(notif) = app_state.notifications.get(*notif_idx) {
                            let time = notif.time_display();
                            let is_pinned = app_state.is_pinned(&notif.id);
                            let is_multi_selected = app_state.is_selected(&notif.id);
                            let colors = TokyoNight::colors();

                            let notification_type = notif.notification_type();
                            let type_icon = Self::get_notification_type_icon(notification_type);
                            let type_style =
                                Self::get_notification_type_style(notification_type, &colors);
                            let reason = notif.reason_enum();
                            let reason_icon = Self::get_notification_reason_icon(reason);
                            let reason_style = Self::get_notification_reason_style(reason, &colors);

                            let title_style = if is_pinned {
                                Style::default().fg(colors.red)
                            } else if notif.is_unread() {
                                self.theme.title
                            } else {
                                self.theme
                                    .title
                                    .fg(colors.fg_dim)
                                    .add_modifier(Modifier::ITALIC)
                            };

                            let checkbox_span = if is_multi_selected {
                                Span::styled(
                                    "✓ ",
                                    Style::default()
                                        .fg(colors.magenta)
                                        .add_modifier(Modifier::BOLD),
                                )
                            } else {
                                Span::styled("  ", Style::default())
                            };

                            let dot_span = if is_pinned {
                                Span::styled(
                                    "󰐃 ",
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                                )
                            } else {
                                Span::styled(
                                    " ",
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                                )
                            };

                            let time_span = Span::styled(
                                format!("{time} "),
                                Style::default().fg(colors.fg_dim),
                            );
                            let type_icon_span = Span::styled(format!("{type_icon} "), type_style);

                            match config.list_layout {
                                crate::config::ListLayout::RightAligned => {
                                    // Inner content width: area minus 2 borders and 2 padding chars
                                    let inner_width = area.width.saturating_sub(4) as usize;
                                    // Fixed left width: checkbox(2) + dot(2) + time + space(1) + type_icon+space(2)
                                    let left_width = 2 + 2 + time.chars().count() + 1 + 2;

                                    let reason_str = format!("{reason}");
                                    let right_text = match notif.subject_number() {
                                        Some(ref n) => format!("#{n} {reason_str}"),
                                        None => reason_str,
                                    };
                                    let right_width = right_text.chars().count();

                                    let available =
                                        inner_width.saturating_sub(left_width + right_width + 1);
                                    let truncated: String =
                                        notif.title().chars().take(available).collect();
                                    let padding_count = inner_width.saturating_sub(
                                        left_width + truncated.chars().count() + right_width,
                                    );

                                    ListItem::new(Line::from(vec![
                                        checkbox_span,
                                        dot_span,
                                        time_span,
                                        type_icon_span,
                                        Span::styled(truncated, title_style),
                                        Span::styled(" ".repeat(padding_count), Style::default()),
                                        Span::styled(right_text, reason_style),
                                    ]))
                                }
                                crate::config::ListLayout::IconOnly => {
                                    let title_text = match notif.subject_number() {
                                        Some(ref n) => format!("#{n} {}", notif.title()),
                                        None => notif.title().to_string(),
                                    };
                                    ListItem::new(Line::from(vec![
                                        checkbox_span,
                                        dot_span,
                                        time_span,
                                        type_icon_span,
                                        Span::styled(format!("{reason_icon} "), reason_style),
                                        Span::styled(title_text, title_style),
                                    ]))
                                }
                                crate::config::ListLayout::TwoLine => {
                                    let line1 = Line::from(vec![
                                        checkbox_span,
                                        dot_span,
                                        Span::styled(notif.title().to_string(), title_style),
                                    ]);
                                    let sub_text = match notif.subject_number() {
                                        Some(ref n) => format!(
                                            "  ↳ #{n} • {time} • {notification_type} • {reason}"
                                        ),
                                        None => {
                                            format!("  ↳ {time} • {notification_type} • {reason}")
                                        }
                                    };
                                    let line2 = Line::from(Span::styled(
                                        sub_text,
                                        Style::default()
                                            .fg(colors.fg_dim)
                                            .add_modifier(Modifier::DIM),
                                    ));
                                    ListItem::new(vec![line1, line2])
                                }
                            }
                        } else {
                            ListItem::new(Line::from("Invalid notification"))
                        }
                    }
                }
            })
            .collect();

        if !items.is_empty() {
            let max_idx = items.len().saturating_sub(1);
            let selected_idx = app_state.selected_index.min(max_idx);
            self.state.select(Some(selected_idx));
        } else {
            self.state.select(None);
        }

        let count = app_state.visible_count();
        let repo_count = app_state
            .tree_items
            .iter()
            .filter(|item| matches!(item, crate::state::TreeItem::RepositoryHeader(_)))
            .count();
        let unread_count = app_state
            .filtered_notifications
            .iter()
            .filter(|&&idx| {
                app_state
                    .notifications
                    .get(idx)
                    .map(|n| n.is_unread())
                    .unwrap_or(false)
            })
            .count();

        let colors = crate::ui::theme::TokyoNight::colors();
        let selection_count = app_state.selection_count();

        let mut title_spans = vec![
            Span::styled(
                format!("󰞏{count} "),
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {unread_count} "),
                Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("󰉋{repo_count} "),
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if selection_count > 0 {
            title_spans.insert(
                0,
                Span::styled(
                    format!("✓{selection_count} "),
                    Style::default()
                        .fg(colors.magenta)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }

        let (left_title_text, left_title_fg, border_style) =
            if let Some(view_name) = app_state.active_view_name() {
                (
                    format!(" 󰎟 {view_name} "),
                    colors.cyan,
                    Style::default().fg(colors.cyan),
                )
            } else {
                (
                    " 󰎟 Notifications ".to_string(),
                    self.theme.highlight_fg,
                    self.theme.border,
                )
            };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(
                        Line::from(left_title_text).left_aligned().style(
                            Style::default()
                                .fg(left_title_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    )
                    .title(Line::from(title_spans).right_aligned())
                    .border_style(border_style)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
            )
            .highlight_style(
                Style::default()
                    .fg(self.theme.highlight_fg)
                    .bg(self.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(list, area, &mut self.state);
    }

    pub fn scroll_up(&mut self, max_items: usize) {
        if max_items > 0 {
            let current = self.state.selected().unwrap_or(0);
            let new = current.saturating_sub(1);
            self.state.select(Some(new));
        }
    }

    pub fn scroll_down(&mut self, max_items: usize) {
        if max_items > 0 {
            let current = self.state.selected().unwrap_or(0);
            let new = (current + 1).min(max_items.saturating_sub(1));
            self.state.select(Some(new));
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    fn get_notification_type_icon(nt: NotificationType) -> &'static str {
        match nt {
            NotificationType::PullRequest => "",
            NotificationType::Issue => "",
            NotificationType::Commit => "󰜘",
            NotificationType::Release => "󰓹",
            NotificationType::Discussion => "󰍦",
            NotificationType::CheckSuite => "",
            NotificationType::RepositoryVulnerabilityAlert => "",
            NotificationType::WorkflowRun => "",
            NotificationType::ActivityEvent => "",
            NotificationType::Unknown => "󰌵",
        }
    }

    fn get_notification_type_style(nt: NotificationType, colors: &TokyoNight) -> Style {
        match nt {
            NotificationType::PullRequest => Style::default()
                .fg(colors.blue)
                .add_modifier(Modifier::BOLD),
            NotificationType::Issue => Style::default()
                .fg(colors.magenta)
                .add_modifier(Modifier::BOLD),
            NotificationType::Commit => Style::default()
                .fg(colors.yellow)
                .add_modifier(Modifier::BOLD),
            NotificationType::Release => Style::default()
                .fg(colors.orange)
                .add_modifier(Modifier::BOLD),
            NotificationType::RepositoryVulnerabilityAlert => {
                Style::default().fg(colors.red).add_modifier(Modifier::BOLD)
            }
            NotificationType::CheckSuite => Style::default()
                .fg(colors.green)
                .add_modifier(Modifier::BOLD),
            NotificationType::Discussion => Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            NotificationType::WorkflowRun => Style::default()
                .fg(colors.orange)
                .add_modifier(Modifier::BOLD),
            NotificationType::ActivityEvent => Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            NotificationType::Unknown => Style::default()
                .fg(colors.fg_dim)
                .add_modifier(Modifier::BOLD),
        }
    }

    fn get_notification_reason_icon(reason: NotificationReason) -> &'static str {
        match reason {
            NotificationReason::Assign => "󰀄",
            NotificationReason::Comment => "󰦨",
            NotificationReason::Mention => "󰆍",
            NotificationReason::ReviewRequested => "󰦩",
            NotificationReason::Subscribed => "󰓹",
            NotificationReason::StateChange => "󰅺",
            NotificationReason::CiActivity => "󰨞",
            NotificationReason::SecurityAlert => "󰌵",
            NotificationReason::Author => "󰀄",
            NotificationReason::TeamMention => "󰓾",
            NotificationReason::Invitation => "󰓾",
            NotificationReason::Manual => "󰐕",
            NotificationReason::ApprovalRequested => "",
            NotificationReason::MemberFeatureRequested => "",
            NotificationReason::SecurityAdvisoryCredit => "",
            NotificationReason::Unknown => "󰐕",
        }
    }

    fn get_notification_reason_style(reason: NotificationReason, colors: &TokyoNight) -> Style {
        match reason {
            NotificationReason::ReviewRequested => Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            NotificationReason::Mention => Style::default()
                .fg(colors.orange)
                .add_modifier(Modifier::BOLD),
            NotificationReason::Assign => Style::default()
                .fg(colors.green)
                .add_modifier(Modifier::BOLD),
            NotificationReason::Comment => Style::default()
                .fg(colors.blue)
                .add_modifier(Modifier::BOLD),
            NotificationReason::SecurityAlert => {
                Style::default().fg(colors.red).add_modifier(Modifier::BOLD)
            }
            NotificationReason::CiActivity => Style::default()
                .fg(colors.blue)
                .add_modifier(Modifier::BOLD),
            NotificationReason::StateChange => Style::default().fg(colors.green),
            NotificationReason::Subscribed => Style::default()
                .fg(colors.magenta)
                .add_modifier(Modifier::BOLD),
            NotificationReason::Author => Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            NotificationReason::ApprovalRequested => Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            _ => Style::default().fg(colors.fg_dim),
        }
    }
}
