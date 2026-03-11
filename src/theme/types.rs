use ratatui::style::Color;

/// Resolved theme colors mapped to ratatui `Color` values.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub background: Color,
    pub background_panel: Color,
    pub background_element: Color,
    pub text: Color,
    pub text_muted: Color,
    pub border: Color,
    #[allow(dead_code)]
    pub border_active: Color,
    pub border_subtle: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
}

impl Default for ThemeColors {
    /// Fallback: the default "opencode" dark theme palette.
    fn default() -> Self {
        Self {
            primary: Color::Rgb(0xfa, 0xb2, 0x83),            // #fab283
            secondary: Color::Rgb(0x5c, 0x9c, 0xf5),          // #5c9cf5
            accent: Color::Rgb(0x9d, 0x7c, 0xd8),             // #9d7cd8
            background: Color::Rgb(0x0a, 0x0a, 0x0a),         // #0a0a0a
            background_panel: Color::Rgb(0x14, 0x14, 0x14),   // #141414
            background_element: Color::Rgb(0x1e, 0x1e, 0x1e), // #1e1e1e
            text: Color::Rgb(0xee, 0xee, 0xee),               // #eeeeee
            text_muted: Color::Rgb(0x80, 0x80, 0x80),         // #808080
            border: Color::Rgb(0x48, 0x48, 0x48),             // #484848
            border_active: Color::Rgb(0x60, 0x60, 0x60),      // #606060
            border_subtle: Color::Rgb(0x3c, 0x3c, 0x3c),      // #3c3c3c
            error: Color::Rgb(0xe0, 0x6c, 0x75),              // #e06c75
            warning: Color::Rgb(0xf5, 0xa7, 0x42),            // #f5a742
            success: Color::Rgb(0x7f, 0xd8, 0x8f),            // #7fd88f
            info: Color::Rgb(0x56, 0xb6, 0xc2),               // #56b6c2
        }
    }
}
