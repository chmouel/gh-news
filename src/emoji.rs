use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MAX_SHORTCODE_LEN: usize = 64;

pub fn expand_shortcodes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while cursor < input.len() {
        let Some(relative_start) = input[cursor..].find(':') else {
            output.push_str(&input[cursor..]);
            break;
        };
        let start = cursor + relative_start;
        output.push_str(&input[cursor..start]);

        let mut end = start + 1;
        let mut valid = true;
        while end < input.len() && end - start - 1 <= MAX_SHORTCODE_LEN {
            match bytes[end] {
                b':' => break,
                byte if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'-') => {
                    end += 1;
                }
                _ => {
                    valid = false;
                    break;
                }
            }
        }

        if valid
            && end < input.len()
            && bytes[end] == b':'
            && end > start + 1
            && end - start - 1 <= MAX_SHORTCODE_LEN
        {
            let shortcode = &input[start + 1..end];
            if let Some(emoji) = emojis::get_by_shortcode(shortcode) {
                output.push_str(emoji.as_str());
            } else {
                output.push_str(&input[start..=end]);
            }
            cursor = end + 1;
        } else {
            output.push(':');
            cursor = start + 1;
        }
    }

    output
}

pub fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub fn truncate_display_width(value: &str, max_width: usize) -> String {
    let mut width = 0;
    value
        .graphemes(true)
        .take_while(|grapheme| {
            let grapheme_width = display_width(grapheme);
            if width + grapheme_width > max_width {
                false
            } else {
                width += grapheme_width;
                true
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_github_shortcodes_and_aliases() {
        assert_eq!(
            expand_shortcodes(":white_check_mark: done :warning: :+1:"),
            "✅ done ⚠️ 👍"
        );
    }

    #[test]
    fn preserves_unknown_and_malformed_shortcodes() {
        assert_eq!(expand_shortcodes(":custom_thing:"), ":custom_thing:");
        assert_eq!(expand_shortcodes(":: :warning"), ":: :warning");
        assert_eq!(
            expand_shortcodes(":unknown:rocket: :rocket:"),
            ":unknown:rocket: 🚀"
        );
        assert_eq!(expand_shortcodes("time: 10:30 :warning:"), "time: 10:30 ⚠️");
    }

    #[test]
    fn handles_consecutive_shortcodes() {
        assert_eq!(expand_shortcodes(":warning::rocket:"), "⚠️🚀");
    }

    #[test]
    fn bounds_malformed_candidates() {
        let malformed = format!(":{} :rocket:", "a".repeat(MAX_SHORTCODE_LEN + 1));
        assert_eq!(
            expand_shortcodes(&malformed),
            format!(":{} 🚀", "a".repeat(MAX_SHORTCODE_LEN + 1))
        );
    }

    #[test]
    fn truncates_by_grapheme_display_width() {
        assert_eq!(truncate_display_width("ab👨‍👩‍👧‍👦cd", 4), "ab👨‍👩‍👧‍👦");
        assert_eq!(display_width(&truncate_display_width("✅ warning", 4)), 4);
    }
}
