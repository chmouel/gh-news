# gh-news

<img width="150" height="150" src="https://github.com/user-attachments/assets/5f01123f-a597-4ba1-b378-b0f72176e89b" align="right" />

GitHub notifications TUI built with Rust and ratatui.

## Screenshot

<img width="1920" height="1279" alt="ghnews" src="https://github.com/user-attachments/assets/d4b57404-ea06-46bf-a0ab-1d46ab4eedb5" />


## Features

- Terminal-based UI for GitHub notifications using Ratatui
- Installs as a native gh CLI extension
- Vim-style navigation with j/k keys
- Multi-select for batch operations on notifications
- Auto-refresh with configurable interva
- Preview notifications
- Regex filtering to filter specific notifications
- Pin important notifications
- Repository grouping with collapsible headers
- Notification hooks for custom commands
- Mute threads and repositories via the GitHub API
- Snooze notifications locally until a chosen time
- Custom actions with command templates
- Mark notifications read/unread individually or in bulk
- Static display mode for scripting and pipelines

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
4. `gh auth token` (queries the `gh` CLI, which reads from the system keyring on modern versions)

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
- `--no-auto-archive` - Disable auto-archiving notifications when navigating
- `--no-cache` - Bypass notification cache and always fetch fresh from the API
- `--state-file <PATH>` - Use a custom state file path (overrides config and default)

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
- `d` - Archive (done) notification — removes from inbox
- `!` - Pin/unpin notification (pinned appear at top)
- `h` - Collapse current repository
- `x` - Open action menu (built-in and custom actions on notifications)

### Multi-select

- `Space` - Toggle selection on notification (magenta checkmark)
- `Esc` - Clear selection (or quit if no selection)
- `Enter` - Open all selected + mark as read
- `o` - Open all selected without marking as read
- `.` - Mark all selected as read
- `d` - Archive all selected
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
- `M` - Toggle auto-mark-read on/off (persisted across sessions)

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

# Structured exclude filters
exclude_types = ["CheckSuite"]           # by type: Issue, PR, Release, CheckSuite, etc.
exclude_reasons = ["subscribed"]         # by reason: subscribed, ci_activity, etc.
exclude_repos = ["noisy-org/*"]          # by repo: exact or glob pattern
exclude_subjects = ["^Bump ", "\\[bot\\]"] # by title: regex patterns (case-insensitive)

# Display
default_preview_mode = "vertical"    # "off", "horizontal", or "vertical"
repos_collapsed = false              # start with repos collapsed
org_grouping = "auto"               # "off", "auto", or "always"

# Behaviour
auto_mark_read = true                # mark notifications read when navigating to them
auto_archive = false                 # archive notifications when navigating away (implies auto_mark_read)

# Notification cache (cached data is shown instantly on startup, then refreshed)
cache_file = ""              # custom cache path (default: ~/.cache/gh-news/notifications_cache.json)

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
notify-send "GitHub: $GH_NEWS_TYPE" "$GH_NEWS_TITLE"
```

**Example: Sound alert**

```toml
on_new_notification_command = "paplay /usr/share/sounds/freedesktop/stereo/message.oga"
```

**Example: Conditional action**

```bash
#!/bin/bash
if [ "$GH_NEWS_REASON" = "review_requested" ]; then
    notify-send -u critical "Review Requested" "$GH_NEWS_TITLE"
fi
```

**Note:** For commands with complex arguments or shell features, use a wrapper script.

### Built-in Actions

The action menu (press `x`) always includes these built-in actions:

| Action | Description |
|--------|-------------|
| Mute Thread | Sets the thread subscription to ignored via the GitHub API. Future notifications for the thread are suppressed until you comment or are `@mentioned` again. |
| Mute Repository | Sets the repository subscription to ignored via the GitHub API. Suppresses all notifications from that repository. |
| Snooze (4 hours) | Hides the notification until 4 hours from now. |
| Snooze (tomorrow 09:00) | Hides the notification until 09:00 the following day. |
| Snooze (next week) | Hides the notification until 09:00 one week from now. |

Snoozed notifications are hidden from the default view and stored locally. They reappear automatically once the snooze period expires. Mute actions are reflected back to GitHub immediately.

All built-in actions support multi-select: select notifications with `Space`, then press `x` and choose an action to apply it to all selected notifications.

### Custom Actions

Define custom actions that can be run on notifications via the action menu (press `x`):

```toml
[[actions]]
name = "Copy URL"
command = "echo {url} | xclip -selection clipboard"

[[actions]]
name = "Open in editor"
command = "code --goto {url}"

[[actions]]
name = "Add to TODO"
command = "echo '* TODO {title}' >> ~/todo.org"

[[actions]]
name = "Browse with fzf"
command = "echo {url} | fzf --preview 'curl -s {}'"
interactive = true  # Suspend TUI for interactive commands
```

Actions support placeholder substitution:

| Placeholder | Description |
|-------------|-------------|
| `{id}` | Notification ID |
| `{title}` | Notification title |
| `{url}` | Web URL for the notification |
| `{repo}` | Repository name (without owner) |
| `{owner}` | Repository owner |
| `{full_name}` | Full repository name (owner/repo) |
| `{type}` | Notification type (Issue, PullRequest, etc.) |
| `{reason}` | Notification reason (mention, review_requested, etc.) |
| `{unread}` | Read status (true/false) |

**Batch Placeholders (plural forms):**

Use plural placeholders to run a single command with all selected notifications:

| Placeholder | Description |
|-------------|-------------|
| `{ids}` | All notification IDs, space-separated |
| `{titles}` | All notification titles, space-separated |
| `{urls}` | All web URLs, space-separated |
| `{repos}` | All repository names, space-separated |
| `{owners}` | All repository owners, space-separated |
| `{full_names}` | All full repository names, space-separated |
| `{types}` | All notification types, space-separated |
| `{reasons}` | All notification reasons, space-separated |

Example batch action:

```toml
[[actions]]
name = "Open all in browser"
command = "firefox {urls}"
interactive = true
```

When you select multiple notifications and run this action, it executes once as `firefox 'url1' 'url2' 'url3'`.

**Action Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `name` | required | Display name in the action menu |
| `command` | required | Command template with placeholders |
| `interactive` | `false` | Suspend TUI and run command with full terminal access (for TUI tools like fzf, vim) |
| `show_output` | `false` | Capture command output and display it in a scrollable TUI popup (incompatible with `interactive`) |

Actions work with multi-select: select multiple notifications with `Space`, then press `x` to run an action on all of them. With singular placeholders, the command runs once per notification. With plural placeholders (e.g., `{urls}`), the command runs once with all values.

## Environment Variables

- `GH_TOKEN` - GitHub personal access token (takes precedence over `GITHUB_TOKEN`)
- `GITHUB_TOKEN` - GitHub personal access token (fallback if `GH_TOKEN` not set)
- `GH_NEWS_AUTO_REFRESH_INTERVAL` - Auto-refresh interval in seconds (default: 120). Set to 0 to disable.

## License

[Apache 2.0](LICENSE)
