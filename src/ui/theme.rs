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
            "dracula_light" => Self::dracula_light(),
            "narna" => Self::narna(),
            "clean_light" => Self::clean_light(),
            "rose_pine_dawn" => Self::rose_pine_dawn(),
            "one_light" => Self::one_light(),
            "everforest_light" => Self::everforest_light(),
            "everforest_dark" => Self::everforest_dark(),
            "one_dark" => Self::one_dark(),
            "rose_pine" => Self::rose_pine(),
            "ayu_mirage" => Self::ayu_mirage(),
            "modern" => Self::modern(),
            "kanagawa" => Self::kanagawa(),
            "solarized_dark" => Self::solarized_dark(),
            "solarized_light" => Self::solarized_light(),
            "gruvbox_light" => Self::gruvbox_light(),
            "monokai" => Self::monokai(),
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

    pub fn dracula_light() -> Self {
        Self {
            bg: Color::Rgb(255, 255, 255),
            bg_dark: Color::Rgb(243, 243, 243),
            bg_highlight: Color::Rgb(208, 215, 222),
            fg: Color::Rgb(36, 41, 47),
            fg_muted: Color::Rgb(110, 119, 129),
            fg_dim: Color::Rgb(87, 96, 106),
            blue: Color::Rgb(198, 219, 229),
            cyan: Color::Rgb(8, 145, 178),
            green: Color::Rgb(5, 150, 105),
            yellow: Color::Rgb(217, 119, 6),
            red: Color::Rgb(220, 38, 38),
            magenta: Color::Rgb(243, 232, 255),
            orange: Color::Rgb(217, 119, 6),
        }
    }

    pub fn narna() -> Self {
        Self {
            bg: Color::Rgb(13, 17, 23),
            bg_dark: Color::Rgb(26, 34, 48),
            bg_highlight: Color::Rgb(48, 54, 61),
            fg: Color::Rgb(230, 237, 243),
            fg_muted: Color::Rgb(139, 148, 158),
            fg_dim: Color::Rgb(110, 118, 129),
            blue: Color::Rgb(65, 173, 255),
            cyan: Color::Rgb(124, 224, 243),
            green: Color::Rgb(63, 185, 80),
            yellow: Color::Rgb(227, 179, 65),
            red: Color::Rgb(244, 112, 103),
            magenta: Color::Rgb(188, 140, 255),
            orange: Color::Rgb(227, 179, 65),
        }
    }

    pub fn clean_light() -> Self {
        Self {
            bg: Color::Rgb(255, 255, 255),
            bg_dark: Color::Rgb(246, 248, 250),
            bg_highlight: Color::Rgb(208, 215, 222),
            fg: Color::Rgb(36, 41, 47),
            fg_muted: Color::Rgb(110, 119, 129),
            fg_dim: Color::Rgb(87, 96, 106),
            blue: Color::Rgb(198, 219, 229),
            cyan: Color::Rgb(5, 152, 188),
            green: Color::Rgb(26, 127, 55),
            yellow: Color::Rgb(154, 103, 0),
            red: Color::Rgb(207, 34, 46),
            magenta: Color::Rgb(130, 80, 223),
            orange: Color::Rgb(154, 103, 0),
        }
    }

    pub fn rose_pine_dawn() -> Self {
        Self {
            bg: Color::Rgb(250, 244, 237),           // Base
            bg_dark: Color::Rgb(255, 250, 243),      // Surface
            bg_highlight: Color::Rgb(242, 233, 225), // Overlay
            fg: Color::Rgb(87, 82, 121),             // Text
            fg_muted: Color::Rgb(152, 147, 165),     // Muted
            fg_dim: Color::Rgb(121, 117, 147),       // Subtle
            blue: Color::Rgb(86, 148, 159),          // Foam
            cyan: Color::Rgb(86, 148, 159),          // Foam
            green: Color::Rgb(40, 105, 131),         // Pine
            yellow: Color::Rgb(234, 157, 52),        // Gold
            red: Color::Rgb(180, 99, 122),           // Love
            magenta: Color::Rgb(144, 122, 169),      // Iris
            orange: Color::Rgb(215, 130, 126),       // Rose
        }
    }

    pub fn one_light() -> Self {
        Self {
            bg: Color::Rgb(250, 250, 250),
            bg_dark: Color::Rgb(229, 229, 230),
            bg_highlight: Color::Rgb(219, 219, 220),
            fg: Color::Rgb(56, 58, 66),
            fg_muted: Color::Rgb(160, 161, 167),
            fg_dim: Color::Rgb(105, 108, 119),
            blue: Color::Rgb(82, 139, 255),
            cyan: Color::Rgb(1, 132, 188),
            green: Color::Rgb(80, 161, 79),
            yellow: Color::Rgb(193, 132, 1),
            red: Color::Rgb(228, 86, 73),
            magenta: Color::Rgb(166, 38, 164),
            orange: Color::Rgb(193, 132, 1),
        }
    }

    pub fn everforest_light() -> Self {
        Self {
            bg: Color::Rgb(253, 246, 227),
            bg_dark: Color::Rgb(234, 228, 202),
            bg_highlight: Color::Rgb(224, 220, 199),
            fg: Color::Rgb(92, 106, 114),
            fg_muted: Color::Rgb(147, 159, 145),
            fg_dim: Color::Rgb(130, 145, 129),
            blue: Color::Rgb(58, 148, 197),
            cyan: Color::Rgb(58, 148, 197),
            green: Color::Rgb(141, 161, 1),
            yellow: Color::Rgb(223, 160, 0),
            red: Color::Rgb(248, 85, 82),
            magenta: Color::Rgb(223, 105, 186),
            orange: Color::Rgb(223, 160, 0),
        }
    }

    pub fn everforest_dark() -> Self {
        Self {
            bg: Color::Rgb(45, 53, 59),
            bg_dark: Color::Rgb(61, 72, 77),
            bg_highlight: Color::Rgb(71, 82, 88),
            fg: Color::Rgb(211, 198, 170),
            fg_muted: Color::Rgb(133, 146, 137),
            fg_dim: Color::Rgb(157, 169, 160),
            blue: Color::Rgb(127, 187, 179),
            cyan: Color::Rgb(131, 192, 146),
            green: Color::Rgb(167, 192, 128),
            yellow: Color::Rgb(219, 188, 127),
            red: Color::Rgb(230, 126, 128),
            magenta: Color::Rgb(214, 153, 182),
            orange: Color::Rgb(230, 152, 117),
        }
    }

    pub fn one_dark() -> Self {
        Self {
            bg: Color::Rgb(40, 44, 52),
            bg_dark: Color::Rgb(62, 68, 82),
            bg_highlight: Color::Rgb(53, 59, 69),
            fg: Color::Rgb(171, 178, 191),
            fg_muted: Color::Rgb(92, 99, 112),
            fg_dim: Color::Rgb(127, 132, 142),
            blue: Color::Rgb(97, 175, 239),
            cyan: Color::Rgb(86, 182, 194),
            green: Color::Rgb(152, 195, 121),
            yellow: Color::Rgb(209, 154, 102),
            red: Color::Rgb(224, 108, 117),
            magenta: Color::Rgb(198, 120, 221),
            orange: Color::Rgb(209, 154, 102),
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            bg: Color::Rgb(25, 23, 36),           // Base
            bg_dark: Color::Rgb(31, 29, 46),      // Surface
            bg_highlight: Color::Rgb(38, 35, 58), // Overlay
            fg: Color::Rgb(224, 222, 244),        // Text
            fg_muted: Color::Rgb(110, 106, 134),  // Muted
            fg_dim: Color::Rgb(144, 140, 170),    // Subtle
            blue: Color::Rgb(156, 207, 216),      // Foam
            cyan: Color::Rgb(156, 207, 216),      // Foam
            green: Color::Rgb(49, 116, 143),      // Pine
            yellow: Color::Rgb(246, 193, 119),    // Gold
            red: Color::Rgb(235, 111, 146),       // Love
            magenta: Color::Rgb(196, 167, 231),   // Iris
            orange: Color::Rgb(235, 188, 186),    // Rose
        }
    }

    pub fn ayu_mirage() -> Self {
        Self {
            bg: Color::Rgb(33, 39, 51),
            bg_dark: Color::Rgb(45, 51, 63),
            bg_highlight: Color::Rgb(62, 75, 89),
            fg: Color::Rgb(217, 215, 206),
            fg_muted: Color::Rgb(92, 103, 115),
            fg_dim: Color::Rgb(112, 122, 140),
            blue: Color::Rgb(92, 207, 230),
            cyan: Color::Rgb(144, 225, 198),
            green: Color::Rgb(186, 230, 126),
            yellow: Color::Rgb(255, 174, 87),
            red: Color::Rgb(255, 51, 51),
            magenta: Color::Rgb(212, 191, 255),
            orange: Color::Rgb(255, 174, 87),
        }
    }

    pub fn modern() -> Self {
        Self {
            bg: Color::Rgb(24, 24, 27),
            bg_dark: Color::Rgb(39, 39, 42),
            bg_highlight: Color::Rgb(63, 63, 70),
            fg: Color::Rgb(250, 250, 250),
            fg_muted: Color::Rgb(113, 113, 122),
            fg_dim: Color::Rgb(161, 161, 170),
            blue: Color::Rgb(139, 92, 246),
            cyan: Color::Rgb(6, 182, 212),
            green: Color::Rgb(16, 185, 129),
            yellow: Color::Rgb(245, 158, 11),
            red: Color::Rgb(239, 68, 68),
            magenta: Color::Rgb(217, 70, 239),
            orange: Color::Rgb(249, 115, 22),
        }
    }

    pub fn kanagawa() -> Self {
        Self {
            bg: Color::Rgb(31, 31, 40),
            bg_dark: Color::Rgb(45, 79, 103),
            bg_highlight: Color::Rgb(34, 50, 73),
            fg: Color::Rgb(220, 215, 186),
            fg_muted: Color::Rgb(114, 113, 105),
            fg_dim: Color::Rgb(200, 192, 147),
            blue: Color::Rgb(126, 156, 216),
            cyan: Color::Rgb(122, 168, 159),
            green: Color::Rgb(118, 148, 106),
            yellow: Color::Rgb(192, 163, 110),
            red: Color::Rgb(195, 64, 67),
            magenta: Color::Rgb(149, 127, 184),
            orange: Color::Rgb(255, 160, 102),
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            bg: Color::Rgb(0, 43, 54),
            bg_dark: Color::Rgb(7, 54, 66),
            bg_highlight: Color::Rgb(88, 110, 117),
            fg: Color::Rgb(147, 161, 161),
            fg_muted: Color::Rgb(88, 110, 117),
            fg_dim: Color::Rgb(101, 123, 131),
            blue: Color::Rgb(38, 139, 210),
            cyan: Color::Rgb(42, 161, 152),
            green: Color::Rgb(133, 153, 0),
            yellow: Color::Rgb(181, 137, 0),
            red: Color::Rgb(220, 50, 47),
            magenta: Color::Rgb(211, 54, 130),
            orange: Color::Rgb(203, 75, 22),
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            bg: Color::Rgb(253, 246, 227),
            bg_dark: Color::Rgb(238, 232, 213),
            bg_highlight: Color::Rgb(228, 221, 199),
            fg: Color::Rgb(88, 110, 117),
            fg_muted: Color::Rgb(147, 161, 161),
            fg_dim: Color::Rgb(131, 148, 150),
            blue: Color::Rgb(38, 139, 210),
            cyan: Color::Rgb(42, 161, 152),
            green: Color::Rgb(133, 153, 0),
            yellow: Color::Rgb(181, 137, 0),
            red: Color::Rgb(220, 50, 47),
            magenta: Color::Rgb(211, 54, 130),
            orange: Color::Rgb(203, 75, 22),
        }
    }

    pub fn gruvbox_light() -> Self {
        Self {
            bg: Color::Rgb(251, 241, 199),
            bg_dark: Color::Rgb(235, 219, 178),
            bg_highlight: Color::Rgb(213, 196, 161),
            fg: Color::Rgb(60, 56, 54),
            fg_muted: Color::Rgb(124, 111, 100),
            fg_dim: Color::Rgb(80, 73, 69),
            blue: Color::Rgb(69, 133, 136),
            cyan: Color::Rgb(104, 157, 106),
            green: Color::Rgb(152, 151, 26),
            yellow: Color::Rgb(215, 153, 33),
            red: Color::Rgb(204, 36, 29),
            magenta: Color::Rgb(177, 98, 134),
            orange: Color::Rgb(214, 93, 14),
        }
    }

    pub fn monokai() -> Self {
        Self {
            bg: Color::Rgb(39, 40, 34),
            bg_dark: Color::Rgb(62, 61, 50),
            bg_highlight: Color::Rgb(117, 113, 94),
            fg: Color::Rgb(248, 248, 242),
            fg_muted: Color::Rgb(117, 113, 94),
            fg_dim: Color::Rgb(165, 159, 133),
            blue: Color::Rgb(102, 217, 239),
            cyan: Color::Rgb(161, 239, 228),
            green: Color::Rgb(166, 226, 46),
            yellow: Color::Rgb(230, 219, 116),
            red: Color::Rgb(249, 38, 114),
            magenta: Color::Rgb(174, 129, 255),
            orange: Color::Rgb(253, 151, 31),
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
