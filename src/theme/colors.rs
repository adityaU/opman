use ratatui::style::Color;

use super::ThemeColors;

pub fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#000000".to_string(),
    }
}

/// Convert a hex color string (e.g. "#fab283") to a ratatui `Color::Rgb`.
pub(crate) fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

pub(crate) fn brighten(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_add(amount),
            g.saturating_add(amount),
            b.saturating_add(amount),
        ),
        other => other,
    }
}

pub(crate) fn darken(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_sub(amount),
            g.saturating_sub(amount),
            b.saturating_sub(amount),
        ),
        other => other,
    }
}

/// Build a 16-entry ANSI color palette derived from the current theme.
///
/// Programs running in PTYs use ANSI indexed colors (0-15) for most of
/// their output (shell prompts, ls colors, git diff, etc.).  The actual
/// RGB values these map to are normally controlled by the terminal
/// emulator's palette, which we cannot override since we render through
/// a `vt100::Parser`.
///
/// Instead we remap `Color::Indexed(0..15)` **at render time** to
/// theme-appropriate RGB values so that every frame automatically
/// reflects the current theme.
pub fn ansi_palette_from_theme(theme: &ThemeColors) -> [Color; 16] {
    // Determine brightness to decide dark vs light base colours.
    let is_dark = match theme.background {
        Color::Rgb(r, g, b) => ((r as u16 + g as u16 + b as u16) / 3) < 128,
        _ => true,
    };

    if is_dark {
        [
            // 0: Black
            theme.background,
            // 1: Red
            theme.error,
            // 2: Green
            theme.success,
            // 3: Yellow
            theme.warning,
            // 4: Blue
            theme.secondary,
            // 5: Magenta
            theme.accent,
            // 6: Cyan
            theme.info,
            // 7: White (normal)
            theme.text_muted,
            // 8: Bright Black (dark grey)
            theme.border,
            // 9: Bright Red
            brighten(theme.error, 30),
            // 10: Bright Green
            brighten(theme.success, 30),
            // 11: Bright Yellow
            brighten(theme.warning, 30),
            // 12: Bright Blue
            brighten(theme.secondary, 30),
            // 13: Bright Magenta
            brighten(theme.accent, 30),
            // 14: Bright Cyan
            brighten(theme.info, 30),
            // 15: Bright White
            theme.text,
        ]
    } else {
        [
            // 0: Black
            theme.text,
            // 1: Red
            darken(theme.error, 30),
            // 2: Green
            darken(theme.success, 30),
            // 3: Yellow
            darken(theme.warning, 30),
            // 4: Blue
            darken(theme.secondary, 30),
            // 5: Magenta
            darken(theme.accent, 30),
            // 6: Cyan
            darken(theme.info, 30),
            // 7: White (normal)
            theme.text_muted,
            // 8: Bright Black (dark grey)
            theme.border,
            // 9: Bright Red
            theme.error,
            // 10: Bright Green
            theme.success,
            // 11: Bright Yellow
            theme.warning,
            // 12: Bright Blue
            theme.secondary,
            // 13: Bright Magenta
            theme.accent,
            // 14: Bright Cyan
            theme.info,
            // 15: Bright White
            theme.background,
        ]
    }
}

/// Post-process a buffer region, replacing ANSI indexed colors (0-15)
/// with theme-derived RGB values.  Call this immediately after
/// `PseudoTerminal::render` on the same `Rect`.
///
/// NOTE: This is now superseded by `term_render::render_screen()` which
/// performs the remapping inline.  Kept for potential future use.
#[allow(dead_code)]
pub fn remap_ansi_colors(
    buf: &mut ratatui::buffer::Buffer,
    area: ratatui::layout::Rect,
    palette: &[Color; 16],
    theme: &ThemeColors,
) {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let cell = &mut buf[(x, y)];

            match cell.fg {
                Color::Indexed(idx) if (idx as usize) < 16 => {
                    cell.set_fg(palette[idx as usize]);
                }
                Color::Reset => {
                    cell.set_fg(theme.text);
                }
                _ => {}
            }
            match cell.bg {
                Color::Indexed(idx) if (idx as usize) < 16 => {
                    cell.set_bg(palette[idx as usize]);
                }
                Color::Reset => {
                    cell.set_bg(theme.background);
                }
                _ => {}
            }
        }
    }
}
