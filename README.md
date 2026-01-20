# gh-news

GitHub notifications TUI built with Rust and ratatui.

<img width="1761" height="1140" alt="image" src="https://github.com/user-attachments/assets/8afafa37-67c0-49d3-9375-1c07cdcfed21" />

## Installation

Install as a `gh` CLI extension (easiest):

```bash
gh extension install chmouel/gh-news
```

Then run it:

```bash
gh news
```

Or install with cargo:

```bash
cargo install --git https://github.com/chmouel/gh-news.git
```

## Setup

You need a GitHub token. The app looks for it in this order:

1. `GH_TOKEN` env var
2. `GITHUB_TOKEN` env var  
3. Your `gh` CLI config at `~/.config/gh/hosts.yml` (or `$XDG_CONFIG_HOME/gh/hosts.yml`)

Easiest way is to just run `gh auth login` if you have the GitHub CLI installed. Otherwise set `GH_TOKEN` to your personal access token.

## Usage

Just run it:

```bash
gh news
```

### Options

- `-a, --all` - Show all notifications (not just unread)
- `-c, --config <PATH>` - Use a custom config file instead of the default
- `-f, --filter <PATTERN>` - Only show notifications matching this regex
- `-n, --max-notifications <N>` - Limit how many to fetch
- `-p, --participating` - Only show notifications where you're participating/mentioned
- `-r, --mark-read` - Mark all notifications as read (non-interactive)
- `-s, --static-display` - Print notifications and exit (for scripts)
- `--no-auto-mark-read` - Disable auto-marking notifications as read when navigating

### Examples

Filter to specific repos:

```bash
gh news --filter "my-org/my-repo"
```

Only things you're involved in:

```bash
gh news --participating
```

Mark everything read:

```bash
gh news --mark-read
```

Static output for scripts:

```bash
gh news --static-display | grep "something"
```

## Keybindings

- `↑`/`↓` or `j`/`k` - Navigate notifications
- `Home`/`End` - Jump to first/last notification
- `PageUp`/`PageDown` - Page navigation (or scroll preview if shown)
- `Enter` - Open notification in browser and mark as read
- `o` - Open notification in browser without marking as read
- `Space` - Toggle repository expansion
- `h` - Collapse current repository
- `t` - Toggle read/unread status
- `M` - Toggle auto-mark-read on scroll
- `Tab` - Cycle preview modes (Off → Horizontal → Vertical)
- `J`/`K` - Scroll preview (line by line)
- `Shift+U`/`Shift+D` - Scroll preview (5 lines)
- `Ctrl+U`/`Ctrl+D` - Scroll preview (page)
- `1`/`2` - Focus pane 1 (list) / pane 2 (preview)
- `Esc` or `q` or `Ctrl+C` - Quit application
- `?` - Show help

## Configuration

gh-news can be configured via a TOML file at `~/.config/gh-news/config.toml`. All options are optional and have sensible defaults. CLI flags take precedence over config file settings.

### Example Config

```toml
# API & Network
auto_refresh_interval = 300  # seconds, 0 to disable
api_timeout = 30             # seconds
max_notifications = 100      # limit notifications fetched
pagination_size = 50         # notifications per API page

# Default filters (same as CLI flags)
show_read = false            # show read notifications (like --all)
participating_only = false   # only participating (like --participating)
default_filter = ""          # regex filter always applied

# Display
default_preview_mode = "vertical"    # "off", "horizontal", or "vertical"
repos_collapsed = false              # start with repos collapsed

# Behaviour
auto_mark_read = true                # mark notifications read when navigating to them

# External commands
browser_command = ""         # custom browser, e.g. "firefox" (uses system default if empty)

# Notification hooks
on_new_notification_command = ""  # command to run when new notifications appear

# GitHub Enterprise (optional)
github_host = "github.com"   # change for GHE, e.g. "github.mycompany.com"
```

### Notification Hooks

Run a custom command when new notifications appear during auto-refresh:

```toml
on_new_notification_command = "/path/to/your/script.sh"
```

The command runs once per new notification with these environment variables:

| Variable | Description |
|----------|-------------|
| `GH_NEWS_ID` | Notification ID |
| `GH_NEWS_TITLE` | Notification title |
| `GH_NEWS_REPO` | Repository name |
| `GH_NEWS_OWNER` | Repository owner |
| `GH_NEWS_TYPE` | Type (Issue, PullRequest, Discussion, etc.) |
| `GH_NEWS_REASON` | Reason (mention, review_requested, comment, etc.) |
| `GH_NEWS_URL` | Web URL (if available) |
| `GH_NEWS_UNREAD` | Read status (true/false) |
| `GH_NEWS_UPDATED_AT` | ISO 8601 timestamp (if available) |

**Example: Desktop notification (Linux)**

```bash
#!/bin/bash
notify-send "GitHub: $GH_NOTIFY_TYPE" "$GH_NOTIFY_TITLE"
```

**Example: Sound alert**

```toml
on_new_notification_command = "paplay /usr/share/sounds/freedesktop/stereo/message.oga"
```

**Example: Conditional action**

```bash
#!/bin/bash
if [ "$GH_NOTIFY_REASON" = "review_requested" ]; then
    notify-send -u critical "Review Requested" "$GH_NOTIFY_TITLE"
fi
```

**Note:** For commands with complex arguments or shell features, use a wrapper script.

## Environment Variables

- `GH_TOKEN` - GitHub personal access token (takes precedence over `GITHUB_TOKEN`)
- `GITHUB_TOKEN` - GitHub personal access token (fallback if `GH_TOKEN` not set)
- `GH_NEWS_AUTO_REFRESH_INTERVAL` - Auto-refresh interval in seconds (default: 300). Set to 0 to disable.

## License

[Apache 2.0](LICENSE)
