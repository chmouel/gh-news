use crate::api::GitHubClient;
use crate::cache;
use crate::config::Config;
use crate::emoji;
use crate::error::Result;
use crate::filter::Filter;
use crate::models::{Notification, NotificationType};
use crate::notifications::{fetch_extra_sources, fetch_notifications, NotificationFetchOptions};
use crate::state_file::AppStateFile;
use crate::ui::theme::ColorPalette;
use crate::RuntimeOptions;
use chrono::Duration;
use ratatui::style::Color;
use serde_json::{json, Value};

/// GitHub mark from the Nerd Font private-use area, matching the glyph the
/// hand-rolled shell scripts use.
const GITHUB_GLYPH: char = '\u{EA84}';

/// Status bars poll on their own timer, so a slightly stale document spares a
/// burst of API calls without ever showing obviously old data.
const CACHE_TTL: Duration = Duration::seconds(60);

/// Emit the status-bar document for `--json` on stdout.
pub fn run(
    config: &Config,
    opts: &RuntimeOptions,
    tooltip_width: usize,
    color: bool,
) -> Result<()> {
    let options_hash = cache::compute_options_hash(
        opts.show_all,
        opts.participating,
        opts.max_notifications,
        config,
    );
    let cache_path = cache::get_json_cache_path()?;

    // Read before any fetch: an entry fetched across an invalidation then
    // carries the old token and is not served as fresh.
    let generation_path = cache::get_json_generation_path()?;
    let generation = cache::read_generation(&generation_path);

    let notifications =
        match cache::load_cache_fresh(&cache_path, &options_hash, CACHE_TTL, &generation) {
            Some(cached) => cached,
            None => {
                let client = GitHubClient::new(config)?;
                let mut fetched = fetch_notifications(
                    &client,
                    NotificationFetchOptions {
                        show_all: opts.show_all,
                        participating: opts.participating,
                        max_notifications: opts.max_notifications,
                        per_page: config.pagination_size,
                    },
                )?;
                let extra = fetch_extra_sources(&client, config, &fetched);
                fetched.extend(extra);

                // A cache write failure must not turn a good tooltip into an error.
                let _ = cache::save_cache_with_generation(
                    &cache_path,
                    &fetched,
                    &options_hash,
                    &generation,
                );
                fetched
            }
        };

    let filter = Filter::from_config(opts.filter_pattern.as_deref(), config)?;
    // Mirror the TUI's local state at read time, so archiving, dismissing or
    // snoozing there shows on the next poll without another fetch.
    let mut notifications = notifications;
    crate::state_file::drop_dismissed(
        &mut notifications,
        &AppStateFile::load_archived_threads().unwrap_or_default(),
        &config.github_host,
    );
    let snoozed = AppStateFile::load_snoozed_notifications().unwrap_or_default();
    let mut shown: Vec<&Notification> = notifications
        .iter()
        .filter(|n| !snoozed.contains_key(&n.id) && filter.matches(n))
        .collect();

    // Extra sources are appended after the API page limit, so `-n` has to be
    // applied once more here to keep the tooltip the size the caller asked for.
    if let Some(max) = opts.max_notifications {
        shown.truncate(max);
    }

    let document = build_document(&shown, tooltip_width, color.then(|| config.color_palette()));
    println!(
        "{}",
        serde_json::to_string(&document)
            .map_err(|e| crate::error::Error::Config(format!("Failed to serialise JSON: {}", e)))?
    );

    Ok(())
}

/// Build the `{text, class, tooltip}` document a status bar consumes: the
/// unread count in `text` and one line per notification in `tooltip`.
///
/// `palette` supplies the colours; `None` leaves every line plain.
pub fn build_document(
    notifications: &[&Notification],
    tooltip_width: usize,
    palette: Option<ColorPalette>,
) -> Value {
    let unread = notifications.iter().filter(|n| n.is_unread()).count();
    let text = if unread == 0 {
        String::new()
    } else {
        format!("{}{}", GITHUB_GLYPH, unread)
    };

    let tooltip = notifications
        .iter()
        .map(|n| tooltip_line(n, tooltip_width, palette.as_ref()))
        .collect::<Vec<_>>()
        .join("\n");

    json!({ "text": text, "class": "", "tooltip": tooltip })
}

fn tooltip_line(
    notification: &Notification,
    tooltip_width: usize,
    palette: Option<&ColorPalette>,
) -> String {
    let mark = if notification.is_unread() { "•" } else { " " };
    let (_, repo) = notification.repo_abbreviated();
    let subject = collapse_whitespace(&emoji::expand_shortcodes(notification.title()));

    let repo = match palette {
        Some(palette) => format!(
            "<span foreground=\"{}\">{}</span>",
            repo_color(&repo, theme_lightness(palette)),
            escape_markup(&repo)
        ),
        None => escape_markup(&repo),
    };

    format!(
        "{} {} {}  {}",
        mark,
        type_span(notification.notification_type(), palette),
        repo,
        escape_markup(&ellipsize(&subject, tooltip_width))
    )
}

/// The type is drawn as a single Nerd Font glyph so it costs one cell. Types
/// that say nothing useful (watched-repo events, workflow noise) render as a
/// blank cell to keep the columns aligned.
fn type_span(notification_type: NotificationType, palette: Option<&ColorPalette>) -> String {
    let glyph = match notification_type {
        NotificationType::PullRequest => '\u{E726}',
        NotificationType::Issue => '\u{F188}',
        NotificationType::RepositoryVulnerabilityAlert => '\u{F132}',
        NotificationType::Discussion => '\u{F086}',
        NotificationType::Release => '\u{F02B}',
        NotificationType::Commit => '\u{E729}',
        NotificationType::CheckSuite => '\u{F046}',
        NotificationType::ActivityEvent
        | NotificationType::WorkflowRun
        | NotificationType::Unknown => return " ".to_string(),
    };

    match palette.and_then(|palette| type_color(notification_type, palette)) {
        Some(color) => format!("<span foreground=\"{}\">{}</span>", color, glyph),
        None => glyph.to_string(),
    }
}

/// Borrow the theme accents the TUI already uses, so the tooltip and the list
/// agree on what a pull request or a security alert looks like.
fn type_color(notification_type: NotificationType, palette: &ColorPalette) -> Option<String> {
    let color = match notification_type {
        NotificationType::PullRequest => palette.green,
        NotificationType::Issue => palette.yellow,
        NotificationType::RepositoryVulnerabilityAlert => palette.red,
        NotificationType::Discussion => palette.blue,
        NotificationType::Release => palette.magenta,
        NotificationType::Commit => palette.orange,
        NotificationType::CheckSuite => palette.cyan,
        _ => return None,
    };

    color_to_hex(lift_to_legible(color, theme_lightness(palette)))
}

/// Several themes carry deliberately muted accents (rose-pine's pine green is a
/// dark teal) which sink into a dark tooltip. Raise the lightness to the floor
/// the repo hashes use, keeping hue and saturation intact.
fn lift_to_legible(color: Color, floor: f32) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };

    let (hue, saturation, lightness) = rgb_to_hsl(r, g, b);
    if lightness >= floor {
        return color;
    }

    let lifted = hsl_to_rgb(hue, saturation, floor);
    Color::Rgb(lifted.0, lifted.1, lifted.2)
}

fn color_to_hex(color: Color) -> Option<String> {
    match color {
        Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
        _ => None,
    }
}

/// Lightness that stays legible on the configured theme: paler hues over a
/// dark background, deeper ones over a light background.
fn theme_lightness(palette: &ColorPalette) -> f32 {
    match palette.bg {
        Color::Rgb(r, g, b) => {
            let luma = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
            if luma < 0.5 {
                0.70
            } else {
                0.40
            }
        }
        _ => 0.70,
    }
}

/// Derive a stable `#rrggbb` from the repo name, so the same repo always wears
/// the same colour across runs and bars without any configuration.
fn repo_color(repo: &str, lightness: f32) -> String {
    const SATURATION: f32 = 0.60;

    let hash = repo.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    });

    hsl_to_hex((hash % 360) as f32, SATURATION, lightness)
}

fn hsl_to_hex(hue: f32, saturation: f32, lightness: f32) -> String {
    let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };

    let offset = lightness - chroma / 2.0;
    (
        (((r + offset) * 255.0).round()).clamp(0.0, 255.0) as u8,
        (((g + offset) * 255.0).round()).clamp(0.0, 255.0) as u8,
        (((b + offset) * 255.0).round()).clamp(0.0, 255.0) as u8,
    )
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    let lightness = (max + min) / 2.0;

    if chroma == 0.0 {
        return (0.0, 0.0, lightness);
    }

    let saturation = chroma / (1.0 - (2.0 * lightness - 1.0).abs());
    let sector = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };

    (sector * 60.0, saturation, lightness)
}

/// Trim a single-line subject to `max_chars` characters, ending in an ellipsis
/// when something had to go.
fn ellipsize(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut shortened: String = text.chars().take(max_chars - 1).collect();
    shortened.push('…');
    shortened
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Status-bar tooltips are Pango markup, so an unescaped `<` in a subject
/// would either vanish or break the whole tooltip.
fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(unread: bool, title: &str) -> Notification {
        typed_notification(unread, title, "PullRequest")
    }

    fn typed_notification(unread: bool, title: &str, subject_type: &str) -> Notification {
        serde_json::from_str(&format!(
            r#"{{
                "id": "1",
                "unread": {},
                "reason": "mention",
                "repository": {{
                    "id": 1,
                    "name": "liseur",
                    "full_name": "chmouel/liseur",
                    "owner": {{ "login": "chmouel", "id": 1, "type": "User" }},
                    "private": false
                }},
                "subject": {{ "title": {:?}, "type": {:?}, "url": null }},
                "updated_at": "2025-01-01T00:00:00Z"
            }}"#,
            unread, title, subject_type
        ))
        .unwrap()
    }

    fn tokyo_night() -> ColorPalette {
        ColorPalette::from_name("tokyo_night")
    }

    #[test]
    fn short_subject_is_untouched() {
        assert_eq!(ellipsize("fix the parser", 50), "fix the parser");
    }

    #[test]
    fn long_subject_is_ellipsized_to_the_budget() {
        let long = "a".repeat(80);
        let shortened = ellipsize(&long, 50);

        assert_eq!(shortened.chars().count(), 50);
        assert!(shortened.ends_with('…'));
    }

    #[test]
    fn zero_width_yields_nothing() {
        assert_eq!(ellipsize("anything", 0), "");
    }

    #[test]
    fn markup_is_escaped() {
        assert_eq!(
            escape_markup("a < b && c > d"),
            "a &lt; b &amp;&amp; c &gt; d"
        );
    }

    #[test]
    fn newlines_collapse_into_one_space() {
        assert_eq!(collapse_whitespace("fix\n  the\tparser"), "fix the parser");
    }

    #[test]
    fn document_counts_unread_and_marks_lines() {
        let unread = notification(true, "Select words inside a definition");
        let read = notification(false, "Merged the release branch");
        let document = build_document(&[&unread, &read], 50, None);

        assert_eq!(document["text"], format!("{}1", GITHUB_GLYPH));
        assert_eq!(document["class"], "");
        assert_eq!(
            document["tooltip"],
            "• \u{e726} liseur  Select words inside a definition\n  \u{e726} liseur  Merged the release branch"
        );
    }

    #[test]
    fn document_without_unread_has_empty_text() {
        let read = notification(false, "Merged the release branch");
        let document = build_document(&[&read], 50, None);

        assert_eq!(document["text"], "");
        assert_eq!(
            document["tooltip"],
            "  \u{e726} liseur  Merged the release branch"
        );
    }

    #[test]
    fn repo_span_wraps_only_the_repo_name() {
        let unread = notification(true, "Recheck dictionary limits");
        let document = build_document(&[&unread], 50, Some(tokyo_night()));
        let tooltip = document["tooltip"].as_str().unwrap();

        assert!(tooltip.contains("<span foreground=\"#"));
        assert!(tooltip.ends_with(">liseur</span>  Recheck dictionary limits"));
    }

    #[test]
    fn each_type_gets_its_own_glyph() {
        let cases = [
            ("PullRequest", '\u{e726}'),
            ("Issue", '\u{f188}'),
            ("RepositoryVulnerabilityAlert", '\u{f132}'),
            ("Discussion", '\u{f086}'),
            ("Release", '\u{f02b}'),
            ("Commit", '\u{e729}'),
            ("CheckSuite", '\u{f046}'),
        ];

        for (subject_type, glyph) in cases {
            let entry = typed_notification(true, "subject", subject_type);
            assert_eq!(
                type_span(entry.notification_type(), None),
                glyph.to_string(),
                "unexpected glyph for {subject_type}"
            );
        }
    }

    #[test]
    fn noisy_types_leave_a_blank_cell() {
        for subject_type in ["ActivityEvent", "WorkflowRun", "Unknown", "NoSuchType"] {
            let entry = typed_notification(true, "subject", subject_type);
            assert_eq!(type_span(entry.notification_type(), None), " ");
        }
    }

    #[test]
    fn type_glyph_is_coloured_from_the_theme() {
        let span = type_span(
            NotificationType::RepositoryVulnerabilityAlert,
            Some(&tokyo_night()),
        );

        assert_eq!(
            span,
            format!(
                "<span foreground=\"{}\">\u{f132}</span>",
                color_to_hex(tokyo_night().red).unwrap()
            )
        );
    }

    #[test]
    fn muted_theme_accents_are_lifted_for_a_dark_tooltip() {
        let rose_pine = ColorPalette::from_name("rose_pine");
        let pine = rose_pine.green;

        let lifted = lift_to_legible(pine, theme_lightness(&rose_pine));
        let (_, _, before) = rgb_to_hsl(49, 116, 143);
        let (_, _, after) = match lifted {
            Color::Rgb(r, g, b) => rgb_to_hsl(r, g, b),
            _ => panic!("expected an RGB colour"),
        };

        assert!(after > before);
        assert!((after - 0.70).abs() < 0.01);
    }

    #[test]
    fn lifted_colours_never_fall_below_the_floor() {
        for theme in ["tokyo_night", "rose_pine", "catppuccin_latte"] {
            let palette = ColorPalette::from_name(theme);
            let floor = theme_lightness(&palette);

            for accent in [
                palette.green,
                palette.yellow,
                palette.red,
                palette.blue,
                palette.magenta,
                palette.orange,
                palette.cyan,
            ] {
                let Color::Rgb(r, g, b) = lift_to_legible(accent, floor) else {
                    panic!("expected an RGB colour")
                };
                let (_, _, lightness) = rgb_to_hsl(r, g, b);

                assert!(lightness >= floor - 0.01, "{theme} dropped below the floor");
            }
        }
    }

    #[test]
    fn colour_is_stable_per_repo_and_differs_between_repos() {
        assert_eq!(repo_color("liseur", 0.70), repo_color("liseur", 0.70));
        assert_ne!(repo_color("liseur", 0.70), repo_color("pipeline", 0.70));
    }

    #[test]
    fn colour_is_a_six_digit_hex() {
        let colour = repo_color("pipelines-as-code", 0.40);

        assert_eq!(colour.len(), 7);
        assert!(colour.starts_with('#'));
        assert_eq!(colour[1..], colour[1..].to_lowercase());
        assert!(u32::from_str_radix(&colour[1..], 16).is_ok());
    }
}
