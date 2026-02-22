use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use ratatui::style::Color;
use serde_json::Value;
use tracing::{debug, warn};

static OPENCODE_THEMES: Dir = include_dir!("$CARGO_MANIFEST_DIR/opencode-themes");

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

/// Deploy embedded opencode theme JSON files to ~/.config/opencode/themes/.
/// Returns Ok(()) on success, Err on failure (non-fatal).
pub fn deploy_embedded_themes() -> Result<()> {
    let themes_dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("opencode/themes");
    std::fs::create_dir_all(&themes_dir)
        .with_context(|| format!("Failed to create themes dir: {}", themes_dir.display()))?;

    let mut count = 0u32;
    for entry in OPENCODE_THEMES.files() {
        if let Some(name) = entry.path().file_name() {
            let dest = themes_dir.join(name);
            std::fs::write(&dest, entry.contents())
                .with_context(|| format!("Failed to write theme: {}", dest.display()))?;
            count += 1;
        }
    }
    tracing::info!(
        "Deployed {} embedded opencode themes to {}",
        count,
        themes_dir.display()
    );
    Ok(())
}

pub fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#000000".to_string(),
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

fn brighten(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_add(amount),
            g.saturating_add(amount),
            b.saturating_add(amount),
        ),
        other => other,
    }
}

fn darken(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_sub(amount),
            g.saturating_sub(amount),
            b.saturating_sub(amount),
        ),
        other => other,
    }
}

impl ThemeColors {
    /// Return environment variables that hint PTY programs about the current
    /// theme so that tools like neovim, gitui, bat, fzf, etc. can pick
    /// matching colors automatically.
    pub fn pty_env_vars(&self) -> Vec<(String, String)> {
        let bg = color_to_hex(self.background);
        let fg = color_to_hex(self.text);
        let bg_panel = color_to_hex(self.background_panel);
        let bg_element = color_to_hex(self.background_element);
        let border = color_to_hex(self.border);
        let primary = color_to_hex(self.primary);
        let secondary = color_to_hex(self.secondary);
        let accent = color_to_hex(self.accent);
        let error = color_to_hex(self.error);
        let warning = color_to_hex(self.warning);
        let success = color_to_hex(self.success);
        let info = color_to_hex(self.info);
        let muted = color_to_hex(self.text_muted);

        let is_dark = match self.background {
            Color::Rgb(r, g, b) => ((r as u16 + g as u16 + b as u16) / 3) < 128,
            _ => true,
        };

        let mut vars: Vec<(String, String)> = vec![
            ("COLORFGBG".into(), format!("{};{}", if is_dark { "15" } else { "0" }, if is_dark { "0" } else { "15" })),
            ("BACKGROUND".into(), bg.clone()),
            ("FOREGROUND".into(), fg.clone()),
            ("NVIM_TUI_ENABLE_TRUE_COLOR".into(), "1".into()),
            ("BAT_THEME".into(), if is_dark { "base16" } else { "GitHub" }.into()),
            ("FZF_DEFAULT_OPTS".into(), format!(
                "--color=bg:{},fg:{},hl:{},bg+:{},fg+:{},hl+:{},info:{},prompt:{},pointer:{},marker:{},spinner:{},header:{},border:{}",
                bg, fg, primary, bg_element, fg, accent, info, primary, accent, success, secondary, muted, border
            )),
            ("LG_ACCENT_COLOR".into(), primary.clone()),
            ("OPENCODE_BG".into(), bg),
            ("OPENCODE_FG".into(), fg),
            ("OPENCODE_BG_PANEL".into(), bg_panel),
            ("OPENCODE_BG_ELEMENT".into(), bg_element),
            ("OPENCODE_BORDER".into(), border),
            ("OPENCODE_PRIMARY".into(), primary),
            ("OPENCODE_SECONDARY".into(), secondary),
            ("OPENCODE_ACCENT".into(), accent),
            ("OPENCODE_ERROR".into(), error),
            ("OPENCODE_WARNING".into(), warning),
            ("OPENCODE_SUCCESS".into(), success),
            ("OPENCODE_INFO".into(), info),
            ("OPENCODE_MUTED".into(), muted),
        ];

        if is_dark {
            vars.push(("VIM_BACKGROUND".into(), "dark".into()));
        } else {
            vars.push(("VIM_BACKGROUND".into(), "light".into()));
        }

        vars
    }
}

/// Load the active OpenCode theme and resolve it into `ThemeColors`.
///
/// Resolution order:
/// 1. Read `~/.config/opencode/config.json` (or `opencode.jsonc`) → `theme` field
/// 2. Look up theme JSON in:
///    a. `~/.config/opencode/themes/<name>.json`   (user custom)
///    b. Built-in themes bundled alongside the opencode binary
///    c. The opencode source tree (development fallback)
/// 3. Parse `defs` + `theme` sections, resolve color references, pick dark mode.
/// 4. Convert hex strings → `ratatui::style::Color::Rgb`.
///
/// On any failure, returns `ThemeColors::default()` so the TUI always works.
pub fn load_theme() -> ThemeColors {
    match try_load_theme() {
        Ok(colors) => colors,
        Err(e) => {
            warn!("Failed to load opencode theme, using defaults: {}", e);
            ThemeColors::default()
        }
    }
}

fn try_load_theme() -> Result<ThemeColors> {
    let (theme_name, theme_mode) = read_active_theme_name()?;
    debug!(theme = %theme_name, mode = %theme_mode, "Resolved active opencode theme");

    let theme_json = load_theme_json(&theme_name)?;
    parse_theme(&theme_json, &theme_mode)
}

/// Read the active theme name and mode from OpenCode's state/config.
///
/// Resolution order (matching OpenCode's own logic):
/// 1. KV store at `~/.local/state/opencode/kv.json` — primary source for TUI-set theme
/// 2. Config files: `~/.config/opencode/opencode.json`, `opencode.jsonc`, `.opencode.json`
/// 3. Falls back to `("opencode", "dark")` if nothing found
fn read_active_theme_name() -> Result<(String, String)> {
    if let Some(kv_result) = read_theme_from_kv() {
        return Ok(kv_result);
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("opencode");

    let config_candidates = [
        config_dir.join("opencode.json"),
        config_dir.join("config.json"),
        config_dir.join("config.jsonc"),
    ];

    let local_candidates = [
        PathBuf::from("opencode.jsonc"),
        PathBuf::from(".opencode.json"),
    ];

    let all_candidates: Vec<&Path> = config_candidates
        .iter()
        .chain(local_candidates.iter())
        .map(|p| p.as_path())
        .collect();

    for path in &all_candidates {
        if path.exists() {
            debug!(path = %path.display(), "Reading opencode config");
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read {}", path.display()))?;

            let clean = strip_jsonc_comments(&content);

            let parsed: Value =
                serde_json::from_str(&clean).context("Failed to parse opencode config")?;

            let theme = parsed
                .get("theme")
                .or_else(|| parsed.pointer("/sync/data/config/theme"))
                .or_else(|| parsed.pointer("/config/theme"))
                .and_then(|v| v.as_str())
                .unwrap_or("opencode")
                .to_string();

            return Ok((theme, "dark".to_string()));
        }
    }

    debug!("No opencode config found, using default theme");
    Ok(("opencode".to_string(), "dark".to_string()))
}

/// Read theme name and mode from the OpenCode KV store.
///
/// The KV store is at `$XDG_STATE_HOME/opencode/kv.json` (typically `~/.local/state/opencode/kv.json`).
/// Fields: `"theme"` (name), `"theme_mode"` ("dark" or "light").
fn read_theme_from_kv() -> Option<(String, String)> {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".local/state")
        });

    let kv_path = state_dir.join("opencode/kv.json");

    if !kv_path.exists() {
        debug!(path = %kv_path.display(), "KV store not found");
        return None;
    }

    debug!(path = %kv_path.display(), "Reading opencode KV store");
    let content = std::fs::read_to_string(&kv_path).ok()?;
    let parsed: Value = serde_json::from_str(&content).ok()?;

    let theme = parsed
        .get("theme")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mode = parsed
        .get("theme_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("dark")
        .to_string();

    match theme {
        Some(name) => {
            debug!(theme = %name, mode = %mode, "Theme from KV store");
            Some((name, mode))
        }
        None => None,
    }
}

/// Load the theme JSON file by name from known locations.
fn load_theme_json(name: &str) -> Result<Value> {
    let mut search_paths = Vec::new();

    // 1. User custom themes
    if let Some(config_dir) = dirs::config_dir() {
        search_paths.push(
            config_dir
                .join("opencode/themes")
                .join(format!("{}.json", name)),
        );
    }

    // 2. Project-local custom themes
    search_paths.push(PathBuf::from(format!(".opencode/themes/{}.json", name)));

    // 3. Development fallback: opencode source tree themes
    //    Walk up from cwd looking for the theme directory
    if let Ok(cwd) = std::env::current_dir() {
        let dev_path = find_opencode_themes_dir(&cwd);
        if let Some(themes_dir) = dev_path {
            search_paths.push(themes_dir.join(format!("{}.json", name)));
        }
    }

    // 4. Hard-coded known development path
    let dev_themes = PathBuf::from("/usr/local/share/opencode/themes");
    search_paths.push(dev_themes.join(format!("{}.json", name)));

    // Try the opencode-gui source tree directly (for development)
    if let Some(home) = dirs::home_dir() {
        // Common development locations
        for candidate in &[
            "workspace/stuff/opencode-gui/server-code/packages/opencode/src/cli/cmd/tui/context/theme",
        ] {
            search_paths.push(home.join(candidate).join(format!("{}.json", name)));
        }
    }

    for path in &search_paths {
        if path.exists() {
            debug!(path = %path.display(), "Loading theme JSON");
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read theme {}", path.display()))?;
            let parsed: Value =
                serde_json::from_str(&content).context("Failed to parse theme JSON")?;
            return Ok(parsed);
        }
    }

    anyhow::bail!(
        "Theme '{}' not found in any search path: {:?}",
        name,
        search_paths
    )
}

/// Walk up from a directory to find the opencode theme directory in a source tree.
fn find_opencode_themes_dir(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        let candidate = current.join("server-code/packages/opencode/src/cli/cmd/tui/context/theme");
        if candidate.is_dir() {
            return Some(candidate);
        }
        // Also check if we're inside the opencode source tree
        let candidate2 = current.join("packages/opencode/src/cli/cmd/tui/context/theme");
        if candidate2.is_dir() {
            return Some(candidate2);
        }
        current = current.parent()?;
    }
}

/// Parse a theme JSON into resolved `ThemeColors`.
fn parse_theme(json: &Value, mode: &str) -> Result<ThemeColors> {
    let defs = json
        .get("defs")
        .and_then(|v| v.as_object())
        .context("Theme JSON missing 'defs' section")?;

    let theme = json
        .get("theme")
        .and_then(|v| v.as_object())
        .context("Theme JSON missing 'theme' section")?;

    let resolve = |field: &str, fallback: &str| -> Color {
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
/// - `{ "dark": "refName", "light": "refName" }` → pick by mode, then resolve ref
/// - `"#hex"` → direct hex color
/// - `"refName"` → look up in defs → should be `"#hex"`
fn resolve_color(
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

/// Convert a hex color string (e.g. "#fab283") to a ratatui `Color::Rgb`.
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

/// Strip single-line (//) and multi-line (/* */) comments from JSONC content.
fn strip_jsonc_comments(input: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_color() {
        assert_eq!(hex_to_color("#fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
        assert_eq!(hex_to_color("#000000"), Color::Rgb(0, 0, 0));
        assert_eq!(hex_to_color("#ffffff"), Color::Rgb(255, 255, 255));
        assert_eq!(hex_to_color("fab283"), Color::Rgb(0xfa, 0xb2, 0x83));
    }

    #[test]
    fn test_strip_jsonc_comments() {
        let input = r#"
{
    // This is a comment
    "theme": "opencode", /* inline */
    "foo": "bar"
}
"#;
        let stripped = strip_jsonc_comments(input);
        assert!(!stripped.contains("// This is a comment"));
        assert!(!stripped.contains("/* inline */"));
        assert!(stripped.contains("\"theme\": \"opencode\","));
    }

    #[test]
    fn test_resolve_color_direct_hex() {
        let defs = serde_json::Map::new();
        let value = Value::String("#fab283".to_string());
        assert_eq!(
            resolve_color(&value, &defs, "dark"),
            Some("#fab283".to_string())
        );
    }

    #[test]
    fn test_resolve_color_ref() {
        let mut defs = serde_json::Map::new();
        defs.insert(
            "darkStep9".to_string(),
            Value::String("#fab283".to_string()),
        );
        let value = Value::String("darkStep9".to_string());
        assert_eq!(
            resolve_color(&value, &defs, "dark"),
            Some("#fab283".to_string())
        );
    }

    #[test]
    fn test_resolve_color_dark_light_object() {
        let mut defs = serde_json::Map::new();
        defs.insert(
            "darkStep9".to_string(),
            Value::String("#fab283".to_string()),
        );
        defs.insert(
            "lightStep9".to_string(),
            Value::String("#3b7dd8".to_string()),
        );

        let value = serde_json::json!({
            "dark": "darkStep9",
            "light": "lightStep9"
        });

        assert_eq!(
            resolve_color(&value, &defs, "dark"),
            Some("#fab283".to_string())
        );
        assert_eq!(
            resolve_color(&value, &defs, "light"),
            Some("#3b7dd8".to_string())
        );
    }
}
