use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gh-news", version)]
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

    /// Print a status-bar JSON document (text/tooltip/class) instead of the TUI
    #[arg(long)]
    pub json: bool,

    /// Characters of each subject kept in the --json tooltip before an ellipsis
    #[arg(long, value_name = "N", default_value_t = 50)]
    pub tooltip_width: usize,

    /// Leave repo names uncoloured in the --json tooltip (also honours NO_COLOR)
    #[arg(long)]
    pub no_color: bool,

    /// Print a static display (non-interactive, cache is bypassed)
    #[arg(short = 's', long)]
    pub static_display: bool,

    /// Bypass notification cache and always fetch fresh from the API
    #[arg(long)]
    pub no_cache: bool,

    /// Path to state file (overrides config and default)
    #[arg(long)]
    pub state_file: Option<PathBuf>,

    /// Validate config, actions, filters, and GitHub authentication, then exit
    #[arg(long)]
    pub check_config: bool,
}
