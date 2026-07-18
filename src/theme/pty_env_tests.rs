use super::*;
use ratatui::style::Color;
use std::collections::HashMap;

fn as_map(theme: &ThemeColors) -> HashMap<String, String> {
    theme.pty_env_vars().into_iter().collect()
}

#[test]
fn pty_env_dark_theme() {
    let theme = ThemeColors::default(); // dark background
    let vars = theme.pty_env_vars();
    assert_eq!(vars.len(), 21);
    let m = as_map(&theme);
    assert_eq!(m.get("COLORFGBG").unwrap(), "15;0");
    assert_eq!(m.get("VIM_BACKGROUND").unwrap(), "dark");
    assert_eq!(m.get("BAT_THEME").unwrap(), "base16");
    assert_eq!(m.get("NVIM_TUI_ENABLE_TRUE_COLOR").unwrap(), "1");
    assert_eq!(m.get("OPENCODE_BG").unwrap(), "#0a0a0a");
    assert_eq!(m.get("OPENCODE_FG").unwrap(), "#eeeeee");
    assert_eq!(m.get("OPENCODE_PRIMARY").unwrap(), "#fab283");
    assert_eq!(m.get("OPENCODE_BG_PANEL").unwrap(), "#141414");
    assert_eq!(m.get("OPENCODE_BG_ELEMENT").unwrap(), "#1e1e1e");
    assert_eq!(m.get("OPENCODE_BORDER").unwrap(), "#484848");
    assert_eq!(m.get("OPENCODE_MUTED").unwrap(), "#808080");
    assert_eq!(m.get("LG_ACCENT_COLOR").unwrap(), "#fab283");
    // FZF opts should embed the bg/fg colors.
    let fzf = m.get("FZF_DEFAULT_OPTS").unwrap();
    assert!(fzf.contains("bg:#0a0a0a"));
    assert!(fzf.contains("fg:#eeeeee"));
    assert!(fzf.starts_with("--color="));
}

#[test]
fn pty_env_light_theme() {
    let mut theme = ThemeColors::default();
    theme.background = Color::Rgb(240, 240, 240); // bright -> light
    let vars = theme.pty_env_vars();
    assert_eq!(vars.len(), 21);
    let m = as_map(&theme);
    assert_eq!(m.get("COLORFGBG").unwrap(), "0;15");
    assert_eq!(m.get("VIM_BACKGROUND").unwrap(), "light");
    assert_eq!(m.get("BAT_THEME").unwrap(), "GitHub");
    assert_eq!(m.get("OPENCODE_BG").unwrap(), "#f0f0f0");
}

#[test]
fn pty_env_non_rgb_background_treated_dark() {
    let mut theme = ThemeColors::default();
    theme.background = Color::White; // non-rgb -> is_dark true
    let m = as_map(&theme);
    assert_eq!(m.get("VIM_BACKGROUND").unwrap(), "dark");
    assert_eq!(m.get("COLORFGBG").unwrap(), "15;0");
    // color_to_hex of a non-rgb color is "#000000"
    assert_eq!(m.get("OPENCODE_BG").unwrap(), "#000000");
}

#[test]
fn pty_env_contains_all_expected_keys() {
    let theme = ThemeColors::default();
    let m = as_map(&theme);
    for key in [
        "COLORFGBG",
        "BACKGROUND",
        "FOREGROUND",
        "NVIM_TUI_ENABLE_TRUE_COLOR",
        "BAT_THEME",
        "FZF_DEFAULT_OPTS",
        "LG_ACCENT_COLOR",
        "OPENCODE_BG",
        "OPENCODE_FG",
        "OPENCODE_BG_PANEL",
        "OPENCODE_BG_ELEMENT",
        "OPENCODE_BORDER",
        "OPENCODE_PRIMARY",
        "OPENCODE_SECONDARY",
        "OPENCODE_ACCENT",
        "OPENCODE_ERROR",
        "OPENCODE_WARNING",
        "OPENCODE_SUCCESS",
        "OPENCODE_INFO",
        "OPENCODE_MUTED",
        "VIM_BACKGROUND",
    ] {
        assert!(m.contains_key(key), "missing env key {key}");
    }
}
