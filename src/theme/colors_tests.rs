use super::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[test]
fn color_to_hex_rgb() {
    assert_eq!(color_to_hex(Color::Rgb(0xfa, 0xb2, 0x83)), "#fab283");
    assert_eq!(color_to_hex(Color::Rgb(0, 0, 0)), "#000000");
    assert_eq!(color_to_hex(Color::Rgb(255, 255, 255)), "#ffffff");
    assert_eq!(color_to_hex(Color::Rgb(1, 2, 3)), "#010203");
}

#[test]
fn color_to_hex_non_rgb_defaults_black() {
    assert_eq!(color_to_hex(Color::White), "#000000");
    assert_eq!(color_to_hex(Color::Reset), "#000000");
    assert_eq!(color_to_hex(Color::Indexed(5)), "#000000");
}

#[test]
fn hex_to_color_variants() {
    assert_eq!(hex_to_color("#fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
    assert_eq!(hex_to_color("fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
    // Too short -> White
    assert_eq!(hex_to_color("#fff"), Color::White);
    assert_eq!(hex_to_color(""), Color::White);
    assert_eq!(hex_to_color("#"), Color::White);
    // Invalid hex digits fall back to 255 per component
    assert_eq!(hex_to_color("#zz0011"), Color::Rgb(255, 0x00, 0x11));
    assert_eq!(hex_to_color("#00zz11"), Color::Rgb(0x00, 255, 0x11));
    assert_eq!(hex_to_color("#0011zz"), Color::Rgb(0x00, 0x11, 255));
    // Longer than 6 -> only first 6 hex chars used
    assert_eq!(hex_to_color("#aabbccdd"), Color::Rgb(0xaa, 0xbb, 0xcc));
}

#[test]
fn brighten_rgb_and_passthrough() {
    assert_eq!(brighten(Color::Rgb(10, 20, 30), 5), Color::Rgb(15, 25, 35));
    // Saturating add
    assert_eq!(
        brighten(Color::Rgb(250, 200, 0), 30),
        Color::Rgb(255, 230, 30)
    );
    // Non-rgb passes through unchanged
    assert_eq!(brighten(Color::White, 30), Color::White);
    assert_eq!(brighten(Color::Indexed(3), 30), Color::Indexed(3));
}

#[test]
fn darken_rgb_and_passthrough() {
    assert_eq!(darken(Color::Rgb(100, 50, 40), 10), Color::Rgb(90, 40, 30));
    // Saturating sub
    assert_eq!(darken(Color::Rgb(5, 10, 0), 30), Color::Rgb(0, 0, 0));
    // Non-rgb passes through unchanged
    assert_eq!(darken(Color::Reset, 30), Color::Reset);
}

#[test]
fn ansi_palette_dark_theme() {
    let theme = ThemeColors::default(); // dark background #0a0a0a
    let pal = ansi_palette_from_theme(&theme);
    // index 0 == background, 15 == text in dark mode
    assert_eq!(pal[0], theme.background);
    assert_eq!(pal[1], theme.error);
    assert_eq!(pal[7], theme.text_muted);
    assert_eq!(pal[8], theme.border);
    assert_eq!(pal[9], brighten(theme.error, 30));
    assert_eq!(pal[15], theme.text);
}

#[test]
fn ansi_palette_light_theme() {
    let mut theme = ThemeColors::default();
    theme.background = Color::Rgb(240, 240, 240); // bright -> light mode
    let pal = ansi_palette_from_theme(&theme);
    assert_eq!(pal[0], theme.text);
    assert_eq!(pal[1], darken(theme.error, 30));
    assert_eq!(pal[9], theme.error);
    assert_eq!(pal[15], theme.background);
}

#[test]
fn ansi_palette_non_rgb_background_is_dark() {
    let mut theme = ThemeColors::default();
    theme.background = Color::Reset; // non-rgb -> treated as dark
    let pal = ansi_palette_from_theme(&theme);
    assert_eq!(pal[0], theme.background);
    assert_eq!(pal[15], theme.text);
}

#[test]
fn remap_ansi_colors_replaces_indexed_and_reset() {
    let area = Rect::new(0, 0, 2, 2);
    let mut buf = Buffer::empty(area);
    let theme = ThemeColors::default();
    let palette = ansi_palette_from_theme(&theme);

    buf[(0, 0)].set_fg(Color::Indexed(1));
    buf[(0, 0)].set_bg(Color::Indexed(2));
    // Reset cells map to theme text/background
    buf[(1, 0)].set_fg(Color::Reset);
    buf[(1, 0)].set_bg(Color::Reset);
    // Indexed >= 16 untouched
    buf[(0, 1)].set_fg(Color::Indexed(200));
    buf[(0, 1)].set_bg(Color::Indexed(201));
    // Rgb (other) untouched
    buf[(1, 1)].set_fg(Color::Rgb(1, 2, 3));
    buf[(1, 1)].set_bg(Color::Rgb(4, 5, 6));

    remap_ansi_colors(&mut buf, area, &palette, &theme);

    assert_eq!(buf[(0, 0)].fg, palette[1]);
    assert_eq!(buf[(0, 0)].bg, palette[2]);
    assert_eq!(buf[(1, 0)].fg, theme.text);
    assert_eq!(buf[(1, 0)].bg, theme.background);
    assert_eq!(buf[(0, 1)].fg, Color::Indexed(200));
    assert_eq!(buf[(0, 1)].bg, Color::Indexed(201));
    assert_eq!(buf[(1, 1)].fg, Color::Rgb(1, 2, 3));
    assert_eq!(buf[(1, 1)].bg, Color::Rgb(4, 5, 6));
}

#[test]
fn remap_ansi_colors_empty_area_noop() {
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
    let theme = ThemeColors::default();
    let palette = ansi_palette_from_theme(&theme);
    // Should not panic with a zero-sized area.
    remap_ansi_colors(&mut buf, area, &palette, &theme);
}
