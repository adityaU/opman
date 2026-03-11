use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use serde_json::Value;
use tracing::{debug, warn};

use super::parsing::{parse_theme, strip_jsonc_comments};
use super::ThemeColors;

static OPENCODE_THEMES: Dir = include_dir!("$CARGO_MANIFEST_DIR/opencode-themes");

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

/// Load the active OpenCode theme and resolve it into `ThemeColors`.
///
/// Resolution order:
/// 1. Read `~/.config/opencode/config.json` (or `opencode.jsonc`) -> `theme` field
/// 2. Look up theme JSON in:
///    a. `~/.config/opencode/themes/<name>.json`   (user custom)
///    b. Built-in themes bundled alongside the opencode binary
///    c. The opencode source tree (development fallback)
/// 3. Parse `defs` + `theme` sections, resolve color references, pick dark mode.
/// 4. Convert hex strings -> `ratatui::style::Color::Rgb`.
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
/// 1. KV store at `~/.local/state/opencode/kv.json` -- primary source for TUI-set theme
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
