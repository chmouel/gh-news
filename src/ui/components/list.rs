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
        state.select(Some(0)); // Initialize with first item selected
        Self {
            state,
            theme: Theme::default(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, app_state: &AppState) {
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

            let paragraph = Paragraph::new(empty_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(
                            Line::from(" 󰨞1 Notifications ").left_aligned().style(
                                Style::default()
                                    .fg(self.theme.highlight_fg)
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
                        .border_style(self.theme.border)
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

                        // Pin icon
                        line.push(Span::styled(
                            "󰐃 ",
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.red)
                            },
                        ));

                        // "Pinned" text
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
                    crate::state::TreeItem::RepositoryHeader(repo_name) => {
                        // Check if this repo is expanded
                        let is_expanded = app_state
                            .expanded_repos
                            .get(repo_name)
                            .copied()
                            .unwrap_or(true);

                        // Count notifications for this repo
                        let notif_count = app_state
                            .filtered_notifications
                            .iter()
                            .filter(|&&idx| {
                                app_state
                                    .notifications
                                    .get(idx)
                                    .map(|n| n.repo_full_name() == repo_name)
                                    .unwrap_or(false)
                            })
                            .count();

                        let mut line = vec![];

                        // Repository icon
                        let colors = TokyoNight::colors();
                        line.push(Span::styled(
                            "",
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.cyan)
                            },
                        ));

                        // Expand/collapse indicator using nerd-font icons
                        // 󰅂 = chevron_down (expanded), 󰅀 = chevron_right (collapsed)
                        let indicator = if is_expanded { " " } else { " " };
                        line.push(Span::styled(
                            indicator,
                            if is_selected {
                                Style::default().fg(self.theme.highlight_fg)
                            } else {
                                Style::default().fg(colors.yellow)
                            },
                        ));

                        // Repository name
                        line.push(Span::styled(
                            format!("{} ", repo_name),
                            if is_selected {
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                self.theme.repo_owner
                            },
                        ));

                        // Notification count as badge
                        let count_style = if is_selected {
                            Style::default()
                                .fg(self.theme.highlight_fg)
                                .bg(self.theme.highlight_bg)
                        } else {
                            Style::default().fg(colors.fg).bg(colors.bg_dark)
                        };
                        line.push(Span::styled(
                            format!(" ({}) ", notif_count),
                            count_style.add_modifier(Modifier::BOLD),
                        ));

                        ListItem::new(Line::from(line))
                    }
                    crate::state::TreeItem::Notification(notif_idx) => {
                        if let Some(notif) = app_state.notifications.get(*notif_idx) {
                            let time = notif.time_display();
                            let is_pinned = app_state.is_pinned(&notif.id);

                            // Build styled line with spans
                            let mut line = vec![];

                            // Indentation for tree structure with nerd-font icon
                            // 󰆍 = subdirectory_arrow_right
                            line.push(Span::styled("  󰆍 ", Style::default().fg(Color::DarkGray)));

                            // Unread/pinned indicator - enhanced styling
                            let colors = TokyoNight::colors();
                            if is_pinned {
                                line.push(Span::styled(
                                    "󰐃 ",
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                                ));
                            } else if notif.is_unread() {
                                line.push(Span::styled(
                                    " ",
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                                ));
                            } else {
                                line.push(Span::styled(
                                    " ",
                                    Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                                ));
                            }

                            // Time with clock icon
                            line.push(Span::styled(
                                format!("{time} "),
                                Style::default().fg(colors.fg_dim),
                            ));

                            // Type with icon and type-specific coloring
                            let notification_type = notif.notification_type();
                            let type_icon = Self::get_notification_type_icon(notification_type);
                            let type_style =
                                Self::get_notification_type_style(notification_type, &colors);
                            line.push(Span::styled(format!("{} ", type_icon), type_style));
                            line.push(Span::styled(
                                format!("{} ", notif.notification_type()),
                                type_style,
                            ));

                            // Reason with icon and reason-specific coloring
                            let reason = notif.reason_enum();
                            let reason_icon = Self::get_notification_reason_icon(reason);
                            let reason_style = Self::get_notification_reason_style(reason, &colors);
                            line.push(Span::styled(format!("{} ", reason_icon), reason_style));
                            line.push(Span::styled(
                                format!("{} ", notif.reason_enum()),
                                reason_style,
                            ));

                            // Title - red if pinned, italic and dimmed if read
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
                            line.push(Span::styled(notif.title(), title_style));

                            ListItem::new(Line::from(line))
                        } else {
                            ListItem::new(Line::from("Invalid notification"))
                        }
                    }
                }
            })
            .collect();

        // Update ListState to match app_state selection
        // Ensure selected_index is within bounds
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
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(
                        Line::from(" 󰎟 Notifications ".to_string())
                            .left_aligned()
                            .style(
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                    )
                    .title(
                        Line::from(vec![
                            Span::styled(
                                format!("󰞏{count} "),
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(" {unread_count} "),
                                Style::default().fg(colors.red).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("󰉋{repo_count} "),
                                Style::default()
                                    .fg(self.theme.highlight_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                        .right_aligned(),
                    )
                    .border_style(self.theme.border)
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

    /// Scroll up using ratatui's native ListState - decrement selection
    pub fn scroll_up(&mut self, max_items: usize) {
        if max_items > 0 {
            let current = self.state.selected().unwrap_or(0);
            let new = current.saturating_sub(1);
            self.state.select(Some(new));
        }
    }

    /// Scroll down using ratatui's native ListState - increment selection
    pub fn scroll_down(&mut self, max_items: usize) {
        if max_items > 0 {
            let current = self.state.selected().unwrap_or(0);
            let new = (current + 1).min(max_items.saturating_sub(1));
            self.state.select(Some(new));
        }
    }

    /// Get the currently selected index from ListState
    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    /// Get icon for notification type
    fn get_notification_type_icon(nt: NotificationType) -> &'static str {
        match nt {
            NotificationType::PullRequest => "",
            NotificationType::Issue => "",
            NotificationType::Commit => "󰜘",
            NotificationType::Release => "󰓹",
            NotificationType::Discussion => "󰍦",
            NotificationType::CheckSuite => "",
            NotificationType::RepositoryVulnerabilityAlert => "",
            NotificationType::Unknown => "󰌵",
        }
    }

    /// Get style for notification type
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
            NotificationType::Unknown => Style::default()
                .fg(colors.fg_dim)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Get icon for notification reason
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
            NotificationReason::Unknown => "󰐕",
        }
    }

    /// Get style for notification reason
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
            _ => Style::default().fg(colors.fg_dim),
        }
    }
}
