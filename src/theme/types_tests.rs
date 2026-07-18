use super::*;
use ratatui::style::Color;

#[test]
fn default_theme_colors_match_opencode_dark() {
    let t = ThemeColors::default();
    assert_eq!(t.primary, Color::Rgb(0xfa, 0xb2, 0x83));
    assert_eq!(t.secondary, Color::Rgb(0x5c, 0x9c, 0xf5));
    assert_eq!(t.accent, Color::Rgb(0x9d, 0x7c, 0xd8));
    assert_eq!(t.background, Color::Rgb(0x0a, 0x0a, 0x0a));
    assert_eq!(t.background_panel, Color::Rgb(0x14, 0x14, 0x14));
    assert_eq!(t.background_element, Color::Rgb(0x1e, 0x1e, 0x1e));
    assert_eq!(t.text, Color::Rgb(0xee, 0xee, 0xee));
    assert_eq!(t.text_muted, Color::Rgb(0x80, 0x80, 0x80));
    assert_eq!(t.border, Color::Rgb(0x48, 0x48, 0x48));
    assert_eq!(t.border_active, Color::Rgb(0x60, 0x60, 0x60));
    assert_eq!(t.border_subtle, Color::Rgb(0x3c, 0x3c, 0x3c));
    assert_eq!(t.error, Color::Rgb(0xe0, 0x6c, 0x75));
    assert_eq!(t.warning, Color::Rgb(0xf5, 0xa7, 0x42));
    assert_eq!(t.success, Color::Rgb(0x7f, 0xd8, 0x8f));
    assert_eq!(t.info, Color::Rgb(0x56, 0xb6, 0xc2));
}

#[test]
fn theme_colors_clone_and_debug() {
    let t = ThemeColors::default();
    let c = t.clone();
    assert_eq!(c.primary, t.primary);
    assert_eq!(c.info, t.info);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("ThemeColors"));
    assert!(dbg.contains("primary"));
}
