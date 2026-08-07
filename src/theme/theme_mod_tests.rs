use super::*;

#[test]
fn public_reexports_are_accessible() {
    // ThemeColors is re-exported at the module root.
    let theme = ThemeColors::default();

    // color_to_hex + ansi_palette_from_theme are re-exported here too.
    assert_eq!(color_to_hex(theme.background), "#0a0a0a");
    let palette = ansi_palette_from_theme(&theme);
    assert_eq!(palette.len(), 16);
    assert_eq!(palette[0], theme.background);
}

#[test]
fn reexported_fn_items_exist() {
    // Reference the re-exported loading fns so the paths are exercised as items.
    let _f: fn() -> ThemeColors = load_theme;
    let _g: fn(&str) -> ThemeColors = load_theme_with_mode;
}
