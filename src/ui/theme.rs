use ratatui::style::{Color, Modifier, Style};

/// Parse a hex colour string (e.g. "#7aa2f7" or "7aa2f7") into a ratatui `Color`.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// A 13-colour palette used by all TUI components.
#[derive(Clone)]
pub struct ColorPalette {
    // Backgrounds
    pub bg: Color,
    pub bg_dark: Color,
    pub bg_highlight: Color,

    // Foregrounds
    pub fg: Color,
    pub fg_muted: Color,
    pub fg_dim: Color,

    // Accent colours
    pub blue: Color,
    pub cyan: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub magenta: Color,
    pub orange: Color,
}

impl ColorPalette {
    /// Resolve a built-in palette by name. Falls back to `tokyo_night` for unknown names.
    pub fn from_name(name: &str) -> Self {
        match name {
            "tokyo_night" => Self::tokyo_night(),
            "catppuccin_mocha" => Self::catppuccin_mocha(),
            "catppuccin_latte" => Self::catppuccin_latte(),
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            "gruvbox_dark" => Self::gruvbox_dark(),
            _ => Self::tokyo_night(),
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            bg: Color::Rgb(26, 27, 38),            // #1a1b26
            bg_dark: Color::Rgb(65, 72, 104),      // #414868
            bg_highlight: Color::Rgb(86, 95, 137), // #565f89
            fg: Color::Rgb(192, 202, 245),         // #c0caf5
            fg_muted: Color::Rgb(169, 177, 214),   // #a9b1d6
            fg_dim: Color::Rgb(154, 165, 206),     // #9aa5ce
            blue: Color::Rgb(122, 162, 247),       // #7aa2f7
            cyan: Color::Rgb(125, 207, 255),       // #7dcfff
            green: Color::Rgb(158, 206, 106),      // #9ece6a
            yellow: Color::Rgb(224, 175, 104),     // #e0af68
            red: Color::Rgb(247, 118, 142),        // #f7768e
            magenta: Color::Rgb(187, 154, 247),    // #bb9af7
            orange: Color::Rgb(255, 158, 100),     // #ff9e64
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),            // #1e1e2e Base
            bg_dark: Color::Rgb(69, 71, 90),       // #45475a Surface1
            bg_highlight: Color::Rgb(88, 91, 112), // #585b70 Surface2
            fg: Color::Rgb(205, 214, 244),         // #cdd6f4 Text
            fg_muted: Color::Rgb(186, 194, 222),   // #bac2de Subtext1
            fg_dim: Color::Rgb(166, 173, 200),     // #a6adc8 Subtext0
            blue: Color::Rgb(137, 180, 250),       // #89b4fa Blue
            cyan: Color::Rgb(137, 220, 235),       // #89dceb Sky
            green: Color::Rgb(166, 227, 161),      // #a6e3a1 Green
            yellow: Color::Rgb(249, 226, 175),     // #f9e2af Yellow
            red: Color::Rgb(243, 139, 168),        // #f38ba8 Red
            magenta: Color::Rgb(203, 166, 247),    // #cba6f7 Mauve
            orange: Color::Rgb(250, 179, 135),     // #fab387 Peach
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            bg: Color::Rgb(239, 241, 245),           // #eff1f5 Base
            bg_dark: Color::Rgb(188, 192, 204),      // #bcc0cc Surface1
            bg_highlight: Color::Rgb(172, 176, 190), // #acb0be Surface2
            fg: Color::Rgb(76, 79, 105),             // #4c4f69 Text
            fg_muted: Color::Rgb(92, 95, 119),       // #5c5f77 Subtext1
            fg_dim: Color::Rgb(108, 111, 133),       // #6c6f85 Subtext0
            blue: Color::Rgb(30, 102, 245),          // #1e66f5 Blue
            cyan: Color::Rgb(4, 165, 229),           // #04a5e5 Sky
            green: Color::Rgb(64, 160, 43),          // #40a02b Green
            yellow: Color::Rgb(223, 142, 29),        // #df8e1d Yellow
            red: Color::Rgb(210, 15, 57),            // #d20f39 Red
            magenta: Color::Rgb(136, 57, 239),       // #8839ef Mauve
            orange: Color::Rgb(254, 100, 11),        // #fe640b Peach
        }
    }

    pub fn nord() -> Self {
        Self {
            bg: Color::Rgb(46, 52, 64),            // #2e3440 Nord0
            bg_dark: Color::Rgb(59, 66, 82),       // #3b4252 Nord1
            bg_highlight: Color::Rgb(76, 86, 106), // #4c566a Nord3
            fg: Color::Rgb(236, 239, 244),         // #eceff4 Nord6
            fg_muted: Color::Rgb(229, 233, 240),   // #e5e9f0 Nord5
            fg_dim: Color::Rgb(216, 222, 233),     // #d8dee9 Nord4
            blue: Color::Rgb(129, 161, 193),       // #81a1c1 Nord9
            cyan: Color::Rgb(136, 192, 208),       // #88c0d0 Nord8
            green: Color::Rgb(163, 190, 140),      // #a3be8c Nord14
            yellow: Color::Rgb(235, 203, 139),     // #ebcb8b Nord13
            red: Color::Rgb(191, 97, 106),         // #bf616a Nord11
            magenta: Color::Rgb(180, 142, 173),    // #b48ead Nord15
            orange: Color::Rgb(208, 135, 112),     // #d08770 Nord12
        }
    }

    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),             // #282a36 Background
            bg_dark: Color::Rgb(68, 71, 90),        // #44475a Current Line
            bg_highlight: Color::Rgb(98, 114, 164), // #6272a4 Comment
            fg: Color::Rgb(248, 248, 242),          // #f8f8f2 Foreground
            fg_muted: Color::Rgb(191, 191, 191),    // #bfbfbf (slightly dimmed fg)
            fg_dim: Color::Rgb(98, 114, 164),       // #6272a4 Comment
            blue: Color::Rgb(189, 147, 249),        // #bd93f9 Purple (Dracula's primary)
            cyan: Color::Rgb(139, 233, 253),        // #8be9fd Cyan
            green: Color::Rgb(80, 250, 123),        // #50fa7b Green
            yellow: Color::Rgb(241, 250, 140),      // #f1fa8c Yellow
            red: Color::Rgb(255, 85, 85),           // #ff5555 Red
            magenta: Color::Rgb(255, 121, 198),     // #ff79c6 Pink
            orange: Color::Rgb(255, 184, 108),      // #ffb86c Orange
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            bg: Color::Rgb(40, 40, 40),           // #282828 bg
            bg_dark: Color::Rgb(60, 56, 54),      // #3c3836 bg1
            bg_highlight: Color::Rgb(80, 73, 69), // #504945 bg2
            fg: Color::Rgb(235, 219, 178),        // #ebdbb2 fg
            fg_muted: Color::Rgb(213, 196, 161),  // #d5c4a1 fg2
            fg_dim: Color::Rgb(189, 174, 147),    // #bdae93 fg3
            blue: Color::Rgb(131, 165, 152),      // #83a598 blue
            cyan: Color::Rgb(142, 192, 124),      // #8ec07c aqua
            green: Color::Rgb(184, 187, 38),      // #b8bb26 green
            yellow: Color::Rgb(250, 189, 47),     // #fabd2f yellow
            red: Color::Rgb(251, 73, 52),         // #fb4934 red
            magenta: Color::Rgb(211, 134, 155),   // #d3869b purple
            orange: Color::Rgb(254, 128, 25),     // #fe8019 orange
        }
    }
}

pub struct Theme {
    pub repo_owner: Style,
    pub title: Style,
    pub border: Style,
    pub status_bar: Style,
    pub help_text: Style,
    pub preview: Style,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
}

impl Theme {
    pub fn from_palette(colors: &ColorPalette) -> Self {
        Self {
            repo_owner: Style::default()
                .fg(colors.cyan)
                .add_modifier(Modifier::BOLD),
            title: Style::default().fg(colors.fg),
            border: Style::default().fg(colors.bg_dark),
            status_bar: Style::default()
                .fg(colors.fg_muted)
                .add_modifier(Modifier::DIM),
            help_text: Style::default().fg(colors.yellow),
            preview: Style::default().fg(colors.fg),
            highlight_bg: colors.bg_highlight,
            highlight_fg: colors.blue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_with_hash() {
        assert_eq!(parse_hex_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("#00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex_color("#7aa2f7"), Some(Color::Rgb(122, 162, 247)));
    }

    #[test]
    fn test_parse_hex_color_without_hash() {
        assert_eq!(parse_hex_color("ff0000"), Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_parse_hex_color_invalid() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }

    #[test]
    fn test_from_name_known() {
        for name in &[
            "tokyo_night",
            "catppuccin_mocha",
            "catppuccin_latte",
            "nord",
            "dracula",
            "gruvbox_dark",
        ] {
            let _palette = ColorPalette::from_name(name);
        }
    }

    #[test]
    fn test_from_name_unknown_falls_back() {
        let fallback = ColorPalette::from_name("nonexistent");
        let tokyo = ColorPalette::tokyo_night();
        assert_eq!(fallback.bg, tokyo.bg);
    }

    #[test]
    fn test_theme_from_palette() {
        let palette = ColorPalette::dracula();
        let _theme = Theme::from_palette(&palette);
    }
}
