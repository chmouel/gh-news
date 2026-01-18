use ratatui::style::{Color, Style};
use crate::ui::theme::Theme;

pub fn format_notification_line(notification: &crate::models::Notification, is_selected: bool, theme: &Theme) -> String {
    let (owner, name) = notification.repo_abbreviated();
    let time = notification.time_display();
    let unread_symbol = if notification.is_unread() { "•" } else { " " };
    let pointer = if is_selected { "▶" } else { "  " };
    
    // We'll use ANSI codes for now, ratatui will handle styling
    format!(
        "{} {} {} {}/{} {} {} {}",
        pointer,
        unread_symbol,
        time,
        owner,
        name,
        notification.notification_type(),
        notification.reason_enum(),
        notification.title()
    )
}
