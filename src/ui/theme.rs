use ratatui::style::{Color, Modifier, Style};

// Tokyo Night color palette
pub struct TokyoNight {
    // Backgrounds
    pub bg: Color,           // #1a1b26 - Deep background
    pub bg_dark: Color,      // #414868 - Darker base
    pub bg_highlight: Color, // #565f89 - Dark base

    // Foregrounds
    pub fg: Color,       // #c0caf5 - Soft white/foreground
    pub fg_muted: Color, // #a9b1d6 - Muted light
    pub fg_dim: Color,   // #9aa5ce - Muted mid

    // Accent colors
    pub blue: Color,    // #7aa2f7 - Strong blue
    pub cyan: Color,    // #7dcfff - Bright blue/cyan
    pub green: Color,   // #9ece6a - Lime green
    pub yellow: Color,  // #e0af68 - Accent gold
    pub red: Color,     // #f7768e - Accent pink/red
    pub magenta: Color, // #bb9af7 - Magenta/purple
    pub orange: Color,  // #ff9e64 - Accent orange
}

impl TokyoNight {
    pub fn colors() -> Self {
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
    pub fn default() -> Self {
        let colors = TokyoNight::colors();

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
