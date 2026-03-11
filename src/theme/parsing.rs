use anyhow::{Context, Result};
use serde_json::Value;

use super::colors::hex_to_color;
use super::ThemeColors;

/// Parse a theme JSON into resolved `ThemeColors`.
pub(crate) fn parse_theme(json: &Value, mode: &str) -> Result<ThemeColors> {
    let defs = json
        .get("defs")
        .and_then(|v| v.as_object())
        .context("Theme JSON missing 'defs' section")?;

    let theme = json
        .get("theme")
        .and_then(|v| v.as_object())
        .context("Theme JSON missing 'theme' section")?;

    let resolve = |field: &str, fallback: &str| -> ratatui::style::Color {
        theme
            .get(field)
            .and_then(|v| resolve_color(v, defs, mode))
            .map(|hex| hex_to_color(&hex))
            .unwrap_or_else(|| hex_to_color(fallback))
    };

    Ok(ThemeColors {
        primary: resolve("primary", "#fab283"),
        secondary: resolve("secondary", "#5c9cf5"),
        accent: resolve("accent", "#9d7cd8"),
        background: resolve("background", "#0a0a0a"),
        background_panel: resolve("backgroundPanel", "#141414"),
        background_element: resolve("backgroundElement", "#1e1e1e"),
        text: resolve("text", "#eeeeee"),
        text_muted: resolve("textMuted", "#808080"),
        border: resolve("border", "#484848"),
        border_active: resolve("borderActive", "#606060"),
        border_subtle: resolve("borderSubtle", "#3c3c3c"),
        error: resolve("error", "#e06c75"),
        warning: resolve("warning", "#f5a742"),
        success: resolve("success", "#7fd88f"),
        info: resolve("info", "#56b6c2"),
    })
}

/// Resolve a theme color value through the defs reference chain.
///
/// Values can be:
/// - `{ "dark": "refName", "light": "refName" }` -> pick by mode, then resolve ref
/// - `"#hex"` -> direct hex color
/// - `"refName"` -> look up in defs -> should be `"#hex"`
pub(crate) fn resolve_color(
    value: &Value,
    defs: &serde_json::Map<String, Value>,
    mode: &str,
) -> Option<String> {
    match value {
        Value::String(s) => {
            if s.starts_with('#') {
                Some(s.clone())
            } else {
                // It's a reference name — look it up in defs
                defs.get(s.as_str())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        }
        Value::Object(map) => {
            // Pick the dark/light variant, then resolve recursively
            let variant = map.get(mode).or_else(|| map.get("dark"));
            variant.and_then(|v| resolve_color(v, defs, mode))
        }
        _ => None,
    }
}

/// Strip single-line (//) and multi-line (/* */) comments from JSONC content.
pub(crate) fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            result.push(c);
            if c == '\\' {
                // Skip escaped character
                if let Some(next) = chars.next() {
                    result.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            result.push(c);
            continue;
        }

        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    // Single-line comment: skip until newline
                    for ch in chars.by_ref() {
                        if ch == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    // Multi-line comment: skip until */
                    chars.next(); // consume the '*'
                    let mut prev = ' ';
                    for ch in chars.by_ref() {
                        if prev == '*' && ch == '/' {
                            break;
                        }
                        if ch == '\n' {
                            result.push('\n');
                        }
                        prev = ch;
                    }
                }
                _ => {
                    result.push(c);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}
