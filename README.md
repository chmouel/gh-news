# gh-news

GitHub notifications TUI built with Rust and ratatui.

<img width="1865" height="1242" alt="Screenshot 2026-01-20 at 14 58 15" src="https://github.com/user-attachments/assets/dae19973-afdc-4b67-abdf-d63ff8f1446a" />

## Installation

Install as a `gh` CLI extension (easiest):

```bash
gh extension install chmouel/gh-news
```

Then run it:

```bash
gh news
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

gh-news shows a loading screen while fetching notifications during start-up and manual refreshes.

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

```bash
gh news --filter "my-org/my-repo" # Filter to specific repos
gh news --participating # Only things you're involved in
gh news --mark-read # Mark everything read:
gh news --static-display | grep "something" # List notifications without TUI
```

## Keybindings

### Navigation
- `↑`/`↓` or `j`/`k` - Navigate notifications, or repository headers when repositories are collapsed
- `Home`/`End` - Jump to first/last notification
- `PageUp`/`PageDown` - Page navigation (or scroll preview if shown)

### Actions
- `Enter` - Open notification in browser and mark as read, or toggle repository collapse on headers
- `o` - Open notification in browser without marking as read
- `.` - Toggle read/unread status
- `!` - Pin/unpin notification (pinned appear at top)
- `h` - Collapse current repository

### Multi-select
- `Space` - Toggle selection on notification (magenta checkmark)
- `Esc` - Clear selection (or quit if no selection)
- `Enter` - Open all selected + mark as read
- `o` - Open all selected without marking as read
- `.` - Mark all selected as read
- `Ctrl+A` - Archive selected (or all if none selected)
- `Ctrl+Alt+A` - Toggle select all notifications in current repository

### View & Filter
- `A` - Toggle showing read notifications
- `E` - Expunge read notifications
- `/` - Filter notifications (type to search, Enter to keep, Esc to clear)
- `Tab` - Cycle preview modes (Off → Horizontal → Vertical)
- `J`/`K` - Scroll preview (line by line)
- `Shift+U`/`Shift+D` - Scroll preview (5 lines)
- `Ctrl+U`/`Ctrl+D` - Scroll preview (page)
- `1`/`2` - Focus pane 1 (list) / pane 2 (preview)
- `M` - Toggle auto-mark-read on scroll

### General
- `Esc` or `q` or `Ctrl+C` - Quit application
- `?` - Show help

### Help
- `↑`/`↓` or `j`/`k` - Scroll help
- `PageUp`/`PageDown` - Page scroll help
- `Home`/`End` - Jump to top/bottom of help
- `/` - Search within help (type to filter, Enter to keep, Esc to clear)

## Configuration

gh-news can be configured via a TOML file at `~/.config/gh-news/config.toml`. All options are optional and have sensible defaults. CLI flags take precedence over config file settings.

### Example Config

See also the example config file [here](./config.example.toml).

```toml
# API & Network
auto_refresh_interval = 120  # seconds, 0 to disable
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
- `GH_NEWS_AUTO_REFRESH_INTERVAL` - Auto-refresh interval in seconds (default: 120). Set to 0 to disable.

## License

[Apache 2.0](LICENSE)
