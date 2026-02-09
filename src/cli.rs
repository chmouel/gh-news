use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gh-news")]
#[command(about = "GitHub notifications TUI built with Rust", long_about = None)]
pub struct Args {
    /// Show all (read/unread) notifications
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Path to config file
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Filter notifications matching a string (REGEX support)
    #[arg(short = 'f', long)]
    pub filter: Option<String>,

    /// Max number of notifications to show
    #[arg(short = 'n', long)]
    pub max_notifications: Option<usize>,

    /// Show only participating or mentioned notifications
    #[arg(short = 'p', long)]
    pub participating: bool,

    /// Mark all notifications as read
    #[arg(short = 'r', long)]
    pub mark_read: bool,

    /// Mark all notifications as read and archive them
    #[arg(long)]
    pub mark_read_archive: bool,

    /// Print a static display (non-interactive)
    #[arg(short = 's', long)]
    pub static_display: bool,

    /// Disable auto-marking notifications as read when navigating
    #[arg(long)]
    pub no_auto_mark_read: bool,

    /// Disable auto-archiving notifications when navigating
    #[arg(long)]
    pub no_auto_archive: bool,

    /// Path to state file (overrides config and default)
    #[arg(long)]
    pub state_file: Option<PathBuf>,
}
