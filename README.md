# gh-news

<img width="150" height="150" src="https://github.com/user-attachments/assets/5f01123f-a597-4ba1-b378-b0f72176e89b" align="right" />

Read and triage GitHub notifications in your terminal.

## Screenshot

<img width="1920" height="1279" alt="ghnews" src="https://github.com/user-attachments/assets/d4b57404-ea06-46bf-a0ab-1d46ab4eedb5" />

## Features

- Install as a `gh` CLI extension
- Navigate with `j`/`k` and select multiple notifications with `Space`
- Fetch updates in the background and apply them when ready
- Preview issues, pull requests, discussions, and commits, with cached details
- Read the comment that triggered a notification in the preview
- Merge pull requests from the preview (`m`, with confirmation)
- Filter with regexes or switch between named views
- Save triage sessions with their own view, filter, and layout
- Pin notifications and collapse repository groups
- Mark notifications read or unread, one at a time or in bulk
- Mute threads and repositories, or snooze notifications until later
- Run custom actions, with hooks for new notifications
- Print notifications for scripts and pipelines
- Check configuration and token access with `--check-config`
- Show progress in terminals that support OSC `9;4`
- Include GitHub Actions workflow runs and activity events (opt-in)

Recognised GitHub shortcodes such as `:white_check_mark:` render as Unicode emoji
when the terminal font supports them. Repository-specific or image-only custom
emoji remain visible as their original `:shortcode:` text.

## Installation

Install as a `gh` CLI extension:

```bash
gh extension install chmouel/gh-news
```

Then run it:

```bash
gh news
```

## Setup

You need a GitHub token. The app looks for it in this order:

1. `GH_TOKEN` environment variable
2. `GITHUB_TOKEN` environment variable
3. Your `gh` CLI config at `~/.config/gh/hosts.yml` (or `$XDG_CONFIG_HOME/gh/hosts.yml`)
4. `gh auth token`, which can read credentials from your system keyring

Run `gh auth login` if you use the GitHub CLI, or set `GH_TOKEN` to a personal access token.

## Usage

gh-news fetches updates in the background. Press `F5` or `Ctrl+R` to fetch updates
on demand, even with auto-refresh disabled. When updates are ready, a box at the
top right prompts you to press the key again to apply them. Automatic background
updates, and any refresh of an empty list, are applied without prompting. Until then, your
list stays as it is. If nothing changed, the status bar reports “Up to date”
and keeps your selection and preview position.

Marking notifications read updates the list at once and syncs with GitHub in
the background. You can keep navigating while requests finish. Failed requests
appear in the status bar; affected notifications return to unread unless you
have changed them again. On exit, gh-news waits for pending read updates to finish.
Local read and archive changes are saved to the cache for the next launch.

Once GitHub confirms a read update, a bar polling `--json` fetches again on
its next poll instead of waiting out its cache.

The loading panel shows the current refresh stage. The status bar reports
refresh errors and shows your remaining API quota when GitHub provides it.

When you archive a notification in the TUI, gh-news remembers the activity you
last saw on that thread. GitHub can keep returning archived threads when asked
for all notifications, so gh-news hides those stale copies across refreshes and
restarts, including with `--all`. New activity on an archived thread brings it
back. Archiving a pinned notification also unpins it.

### Options

- `-a, --all` - Show all notifications (not just unread)
- `-c, --config <PATH>` - Use a custom config file instead of the default
- `-f, --filter <PATTERN>` - Only show notifications matching this regex (matched against repo, title, type, reason, and author)
- `-n, --max-notifications <N>` - Limit how many to fetch
- `-p, --participating` - Only show notifications where you're participating/mentioned
- `-r, --mark-read` - Mark all notifications as read (non-interactive)
- `--mark-read-archive` - Mark all notifications as read and archive them (non-interactive)
- `-s, --static-display` - Print notifications and exit (for scripts)
- `--json` - Print a status-bar JSON document (`text`, `class`, `tooltip`) and exit, for waybar and friends
- `--tooltip-width <N>` - Characters of each subject kept in the `--json` tooltip before an ellipsis (default: 50)
- `--no-color` - Leave repo names and type glyphs uncoloured in the `--json` tooltip (also honours `NO_COLOR`)
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

### Status bar integration

`--json` prints one document on stdout and exits, so a status bar can poll it
on a timer. `text` carries the unread count next to a GitHub glyph, `tooltip`
holds one line per notification, and each subject is trimmed to
`--tooltip-width` characters with an ellipsis. Unread entries are marked `•`.
Everything else (`-p`, `-f`, `-n`, filters, `exclude_*`) applies as usual, and
`--json` keeps its own 60-second cache (empty results included) so a tight
polling interval does not burn API quota.

The bar is an independent query with its own options, so its count can differ
from the TUI when the two are started with different flags. It does share the
TUI's local state: threads you archived, dismissed or snoozed there disappear
from the bar on its next poll, because the bar applies the saved state itself
(a confirmed archive on GitHub also makes it fetch again). Local-only changes, such
as toggling a notification back to unread, are not mirrored. A manual refresh in the
TUI, an automatic refresh that found changes, and a read update confirmed by
GitHub make the bar fetch again on its next poll. Both processes must use the
same state file.

Each line reads `• <type glyph> <repo>  <subject>`:

- The type glyph is one Nerd Font cell: pull request, bug for issues, shield for
  security alerts, comments for discussions, tag for releases, commit, and a
  check square for check suites. Events, workflow runs, and unknown types leave
  the cell blank so the columns stay aligned.
- Types take their colour from the configured theme's accents, lifted to a
  minimum lightness so muted palettes such as rose-pine stay legible on a dark
  tooltip.
- The repo name is coloured from a hash of its name, so the same repository
  always wears the same hue. `--no-color`, or a non-empty `NO_COLOR`, prints
  plain text instead.

```json
{"text":"\uea843","class":"","tooltip":"•  <span foreground=\"#8dc2d8\">liseur</span>  Select words…\n  <span foreground=\"#8ce085\">pipeline</span>  Fix CVEs…"}
```

Waybar needs `"return-type": "json"` and `"tooltip": true` on the module:

```jsonc
"custom/github_notifications": {
  "format": "{}",
  "return-type": "json",
  "tooltip": true,
  "interval": 30,
  "exec": "gh-news --json --participating --max-notifications 10"
}
```

Subjects are escaped for Pango markup, so a `<` in a title is displayed rather
than swallowed.

## Keybindings

### Navigation

- `↑`/`↓` or `j`/`k` - Navigate notifications, or repository headers when repositories are collapsed
- `Home`/`End` - Jump to first/last notification
- `PageUp`/`PageDown` - Page navigation (or scroll preview if shown)

### Actions

Press `Space` to select notifications, shown with a magenta checkmark. Actions
then apply to the selection.

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
- `v` - Expand/collapse pull request description
- `1`/`2` - Focus pane 1 (list) / pane 2 (preview)
- `M` - Toggle auto-mark-read on/off (persisted across sessions)

### General

- `F5` or `Ctrl+R` - Fetch fresh updates, or apply them when ready
- `Esc` or `q` or `Ctrl+C` - Quit application
- `?` - Show help

### Help

- `↑`/`↓` or `j`/`k` - Scroll help
- `PageUp`/`PageDown` - Page scroll help
- `Home`/`End` - Jump to top/bottom of help
- `/` - Search within help (type to filter, Enter to keep, Esc to clear)

## Configuration

Put settings in `~/.config/gh-news/config.toml`. Every setting is optional;
CLI flags override the file.

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

Run a command for each new notification when you apply refreshed content:

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

For desktop alerts on review requests:

```bash
#!/bin/bash
if [ "$GH_NEWS_REASON" = "review_requested" ]; then
    notify-send -u critical "Review Requested" "$GH_NEWS_TITLE"
fi
```

Use a wrapper script for commands that need shell features or complex arguments.

### Built-in Actions

Press `x` to open the action menu:

| Action | Description |
| -------- | ------------- |
| Mute Thread | Ignore the thread until you comment or receive another `@mention`. |
| Mute Repository | Stop notifications from the repository. |
| Snooze (4 hours) | Hides the notification until 4 hours from now. |
| Snooze (tomorrow 09:00) | Hides the notification until 09:00 the following day. |
| Snooze (next week) | Hides the notification until 09:00 one week from now. |

Snoozes are stored locally. Notifications reappear after the snooze expires
and you apply a refresh. Mute actions update your GitHub subscription.

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

Press `V` to switch filter presets. Choose a built-in view or add your own in
`config.toml`.

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

Your views follow the built-in views in the picker. Choose `0. Default` to
restore your starting filter. Search with `/` within the active view; `Ctrl+A`
acts on the visible results.

### Saved Triage Sessions

Press `S` to switch saved sessions. Each session can set a view and an extra
regex filter, show read notifications, and choose the preview layout and
repository collapse state.

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

## Author

[![Sponsor](https://img.shields.io/badge/Sponsor-❤️-ff69b4?style=for-the-badge&logo=github)](https://github.com/sponsors/chmouel)

### Chmouel Boudjnah

- Fediverse - [@chmouel@chmouel.com](https://fosstodon.org/@chmouel)
- Twitter - [@chmouel](https://twitter.com/chmouel)
- Blog  - [https://blog.chmouel.com](https://blog.chmouel.com)

## License

[Apache 2.0](LICENSE)
