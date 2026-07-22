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
- Auto-refresh with configurable interval
- Preview notifications with rich details (GraphQL-powered for Issues, PRs, Discussions, Commits)
- Quick-merge pull requests straight from the preview (`m`, with confirmation)
- When a notification is triggered by a comment, the preview shows that comment instead of the issue/PR description
- Persisted preview cache for faster repeated detail views
- Regex filtering to filter specific notifications
- Pin important notifications
- Repository grouping with collapsible headers
- Notification hooks for custom commands
- Mute threads and repositories via the GitHub API
- Snooze notifications locally until a chosen time
- Custom actions with command templates
- Named views for instant filter preset switching
- Saved triage sessions for switching complete work contexts
- Mark notifications read/unread individually or in bulk
- Static display mode for scripting and pipelines
- Configuration doctor for validating filters, actions, token access, and views
- Progress reporting for supporting terminals via OSC `9;4`
- GitHub Actions workflow run notifications (opt-in)
- GitHub Activity Events feed (opt-in)

Recognised GitHub shortcodes such as `:white_check_mark:` render as Unicode emoji
when the terminal font supports them. Repository-specific or image-only custom
emoji remain visible as their original `:shortcode:` text.

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

Press `Ctrl+R` to force a refresh. gh-news shows the current refresh stage in the loading panel while it refreshes notifications and any enabled extra sources. When GitHub returns rate-limit headers, the status bar shows the remaining API quota. Local actions such as marking notifications read or archiving them update the on-disk cache straight away, so dismissed notifications do not reappear on the next startup. Should a background refresh fail, the status bar reports the error rather than failing silently.

### Options

- `-a, --all` - Show all notifications (not just unread)
- `-c, --config <PATH>` - Use a custom config file instead of the default
- `-f, --filter <PATTERN>` - Only show notifications matching this regex (matched against repo, title, type, reason, and author)
- `-n, --max-notifications <N>` - Limit how many to fetch
- `-p, --participating` - Only show notifications where you're participating/mentioned
- `-r, --mark-read` - Mark all notifications as read (non-interactive)
- `--mark-read-archive` - Mark all notifications as read and archive them (non-interactive)
- `-s, --static-display` - Print notifications and exit (for scripts)
- `--check-config` - Validate config, custom actions, filters, views, and GitHub authentication, then exit
- `--no-cache` - Bypass notification cache and always fetch fresh from the API
- `--state-file <PATH>` - Use a custom state file path (overrides config and default)

### Examples

```bash
gh news --filter "my-org/my-repo" # Filter to specific repos
gh news --participating # Only things you're involved in
gh news --mark-read # Mark everything read
gh news --check-config # Check config and token access
gh news --static-display | grep "something" # List notifications without TUI
```

## Keybindings

### Navigation

- `↑`/`↓` or `j`/`k` - Navigate notifications, or repository headers when repositories are collapsed
- `Home`/`End` - Jump to first/last notification
- `PageUp`/`PageDown` - Page navigation (or scroll preview if shown)

### Actions

Multi-select first with `Space` (magenta checkmark); the actions below then apply to the whole selection instead of just the current notification.

- `Enter` - Open in browser and mark as read, or toggle repository collapse on headers
- `o` - Open in browser without marking as read
- `O` - Open URL menu (open/copy/print) for the current selection
- `.` - Toggle read/unread status
- `d` - Archive (done), removing it from the inbox
- `m` - Merge the previewed pull request (confirmation dialog; method set by `merge_method` in config, default squash)
- `!` - Pin/unpin notification (pinned appear at top)
- `h` - Collapse current repository
- `x` - Open action menu (built-in and custom actions on notifications)
- `Esc` - Clear selection (or quit if no selection)
- `Ctrl+A` - Act on selected notifications, or on the current filtered list if none are selected
- `Ctrl+Alt+A` - Toggle select all notifications in current repository

### View & Filter

- `A` - Toggle showing read notifications
- `E` - Expunge read notifications
- `/` - Filter notifications (type to search, Enter to keep, Esc to clear)
- `V` - Switch named view (built-in and custom filter presets)
- `S` - Switch saved triage session
- `Tab` - Cycle preview modes (Off → Horizontal → Vertical)
- `J`/`K` - Scroll preview (line by line)
- `Shift+U`/`Shift+D` - Scroll preview (5 lines)
- `Ctrl+U`/`Ctrl+D` - Scroll preview (page)
- `c` - Expand/collapse pull request CI checks
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

Full reference with every option and its default: [config.example.toml](./config.example.toml).

```toml
# API & Network
auto_refresh_interval = 120  # seconds, 0 to disable
max_notifications = 100      # limit notifications fetched

# Default filters (same as CLI flags)
show_read = false            # show read notifications (like --all)
exclude_repos = ["noisy-org/*"]

# Theme
theme = "tokyo_night"        # see Themes section below for all options

# Behaviour
auto_mark_read = false       # mark notifications read when navigating to them
```

### Themes

gh-news ships with 21 built-in colour themes, both dark and light. Set the
`theme` key in your config to switch:

| Name | Style |
| ------ | ------- |
| `tokyo_night` (default) | Dark blue with soft white text |
| `catppuccin_mocha` | Warm dark (Catppuccin dark variant) |
| `catppuccin_latte` | Light (Catppuccin light variant) |
| `nord` | Arctic blue-grey |
| `dracula` | Dark with vivid accents |
| `gruvbox_dark` | Retro warm colours |

See [config.example.toml](./config.example.toml) for the full list, including
`one_light`, `rose_pine`, `solarized_dark`, `kanagawa`, `monokai`, and more.

```toml
theme = "catppuccin_mocha"
```

You can also override individual palette colours on top of any theme using the
`[theme_colors]` table. Each value is a hex string (`"#rrggbb"` or `"rrggbb"`):

```toml
theme = "nord"

[theme_colors]
blue = "#7aa2f7"
red  = "#ff0000"
```

Available colour fields: `bg`, `bg_dark`, `bg_highlight`, `fg`, `fg_muted`,
`fg_dim`, `blue`, `cyan`, `green`, `yellow`, `red`, `magenta`, `orange`.

#### Theme Screenshot

**rose_pine_dawn**

<img width="3730" height="2484" alt="rose_pine_dawn" src="https://github.com/user-attachments/assets/0179bf1c-1ef8-437b-86e7-60bdf993f423" />

### Notification Hooks

Run a custom command when new notifications appear during auto-refresh:

```toml
on_new_notification_command = "/path/to/your/script.sh"
```

The command runs once per new notification with these environment variables:

| Variable | Description |
| ---------- | ------------- |
| `GH_NEWS_ID` | Notification ID |
| `GH_NEWS_TITLE` | Notification title |
| `GH_NEWS_REPO` | Repository name |
| `GH_NEWS_OWNER` | Repository owner |
| `GH_NEWS_TYPE` | Type (Issue, PullRequest, Discussion, etc.) |
| `GH_NEWS_REASON` | Reason (mention, review_requested, comment, etc.) |
| `GH_NEWS_URL` | Web URL (if available) |
| `GH_NEWS_UNREAD` | Read status (true/false) |
| `GH_NEWS_UPDATED_AT` | ISO 8601 timestamp (if available) |

**Example: Notify only on review requests**

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
| -------- | ------------- |
| Mute Thread | Sets the thread subscription to ignored via the GitHub API. Future notifications for the thread are suppressed until you comment or are `@mentioned` again. |
| Mute Repository | Sets the repository subscription to ignored via the GitHub API. Suppresses all notifications from that repository. |
| Snooze (4 hours) | Hides the notification until 4 hours from now. |
| Snooze (tomorrow 09:00) | Hides the notification until 09:00 the following day. |
| Snooze (next week) | Hides the notification until 09:00 one week from now. |

Snoozed notifications are hidden from the default view and stored locally. They reappear automatically once the snooze period expires. Mute actions are reflected back to GitHub immediately.

### Custom Actions

Define custom actions that can be run on notifications via the action menu (press `x`):

```toml
[[actions]]
name = "Copy URL"
command = "echo {url} | xclip -selection clipboard"
priority = 1  # Lower numbers sort earlier in the action menu

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

| Placeholder | Description | Plural form (batch, space-separated) |
| ------------- | ------------- | ------------- |
| `{id}` | Notification ID | `{ids}` |
| `{title}` | Notification title | `{titles}` |
| `{number}` | PR/issue/discussion number (empty for other types) | - |
| `{url}` | Web URL for the notification | `{urls}` |
| `{repo}` | Repository name (without owner) | `{repos}` |
| `{owner}` | Repository owner | `{owners}` |
| `{full_name}` | Full repository name (owner/repo) | `{full_names}` |
| `{type}` | Notification type (Issue, PullRequest, etc.) | `{types}` |
| `{reason}` | Notification reason (mention, review_requested, etc.) | `{reasons}` |
| `{unread}` | Read status (true/false) | - |

Plural placeholders run the command once with all selected notifications instead of once per notification. Example:

```toml
[[actions]]
name = "Open all in browser"
command = "firefox {urls}"
interactive = true
```

When you select multiple notifications and run this action, it executes once as `firefox 'url1' 'url2' 'url3'`.

**Action Options:**

| Option | Default | Description |
| -------- | --------- | ------------- |
| `name` | required | Display name in the action menu |
| `command` | required | Command template with placeholders |
| `priority` | unset | Lower numbers sort earlier in the action menu |
| `interactive` | `false` | Suspend TUI and run command with full terminal access (for TUI tools like fzf, vim) |
| `show_output` | `false` | Capture command output and display it in a scrollable TUI popup (incompatible with `interactive`) |

### Named Views

Named views are saved filter presets you can switch between instantly with `V`. Several built-in views are always available, and you can add your own in `config.toml`.

**Built-in views:**

| View | What it shows |
| ------ | --------------- |
| Participating | Everything except passive subscriptions and CI noise (`subscribed`, `ci_activity` reasons excluded) |
| Mentions | Direct `@mention` and team mention notifications |
| Review Requests | PRs where your review has been requested |
| Assigned | Issues and PRs assigned to you |
| My Activity | Notifications on threads you opened or created |
| Security | Security alerts and advisories |
| Dependabot | Dependabot version-bump PRs (titles matching "Bump X from Y to Z") and any notification where "dependabot" appears |
| Bots | Activity from bots whose login ends in `[bot]` (matched against the enriched author field) |

**Custom views:**

Define your own in `config.toml` using `[[views]]` sections. All filter fields are optional; unset fields inherit from the global config defaults.

```toml
[[views]]
name = "Fires"
exclude_types = ["Release", "RepositoryVulnerabilityAlert"]
exclude_reasons = ["subscribed"]

[[views]]
name = "My Org"
exclude_repos = ["*"]          # exclude everything …
filter = "my-org/"             # … that doesn't match this repo pattern

[[views]]
name = "Dependabot"
filter = "dependabot"
```

**View fields:**

| Field | Description |
| ------- | ------------- |
| `name` | Display name shown in the picker (required) |
| `filter` | Regex applied to `repo title type reason author` (author populated by background enrichment) |
| `exclude_types` | Override global `exclude_types` for this view |
| `exclude_reasons` | Override global `exclude_reasons` for this view |
| `exclude_repos` | Override global `exclude_repos` for this view (glob patterns) |
| `exclude_subjects` | Override global `exclude_subjects` for this view (regex, case-insensitive) |

User-defined views appear after the built-in views in the picker. Selecting `0. Default` clears the active view and restores the session's base filter. The `/` search works within the active view, and bulk actions (`Ctrl+A`) apply to the visible filtered result set.

### Saved Triage Sessions

Saved sessions are named work contexts you can switch to with `S`. They can activate a view, layer on an extra regex filter, change whether read notifications are shown, set the preview mode, and collapse repositories.

```toml
[[sessions]]
name = "Reviews"
view = "Review Requests"
filter = "my-org/"
show_read = false
preview_mode = "vertical"
repos_collapsed = true
```

**Session fields:**

| Field | Description |
| ------- | ------------- |
| `name` | Display name shown in the session picker (required) |
| `view` | Built-in or custom view name to activate |
| `filter` | Extra regex filter layered on top of the selected view/default filter |
| `show_read` | Override whether read notifications are fetched and shown |
| `preview_mode` | Override preview mode: `off`, `horizontal`, or `vertical` |
| `repos_collapsed` | Start the session with repositories collapsed or expanded |

## Environment Variables

- `GH_TOKEN` - GitHub personal access token (takes precedence over `GITHUB_TOKEN`)
- `GITHUB_TOKEN` - GitHub personal access token (fallback if `GH_TOKEN` not set)
- `GH_NEWS_AUTO_REFRESH_INTERVAL` - Auto-refresh interval in seconds (default: 120). Set to 0 to disable.

## License

[Apache 2.0](LICENSE)
