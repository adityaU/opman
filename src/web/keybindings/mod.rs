//! Persistence for the web UI keybinding config.
//!
//! Lives at `~/.config/opman/keybindings.json`, alongside `acp.json`. Distinct
//! from the `[keybindings]` table in `config.toml`, which belongs to the
//! terminal UI and has its own which-key tree.

mod types;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

pub use types::{Diagnostic, KeybindingsConfig, KeybindingsResponse};

/// `$OPMAN_KEYBINDINGS_CONFIG`, else `~/.config/opman/keybindings.json`.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("OPMAN_KEYBINDINGS_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    dirs::config_dir().map(|dir| dir.join("opman").join("keybindings.json"))
}

/// Read the config, degrading to defaults with a diagnostic rather than failing.
///
/// A missing file is not a problem — it is the common case — so it yields
/// defaults and no diagnostic. A malformed one yields defaults *and* a
/// diagnostic carrying the parse position, which the keybindings view renders.
pub fn load() -> KeybindingsResponse {
    let path = config_path();
    let display = path.as_ref().map(|p| p.display().to_string());

    let Some(path) = path else {
        return response(KeybindingsConfig::default(), Vec::new(), display);
    };

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return response(KeybindingsConfig::default(), Vec::new(), display);
        }
        Err(err) => {
            let message = format!("could not read keybindings.json: {err}");
            return response(KeybindingsConfig::default(), vec![plain(message)], display);
        }
    };

    match serde_json::from_str::<KeybindingsConfig>(&raw) {
        Ok(config) => response(config, Vec::new(), display),
        Err(err) => {
            let diagnostic = Diagnostic {
                message: err.to_string(),
                line: u32::try_from(err.line()).ok(),
                column: u32::try_from(err.column()).ok(),
            };
            tracing::warn!(path = %path.display(), "ignoring malformed keybindings.json: {err}");
            response(KeybindingsConfig::default(), vec![diagnostic], display)
        }
    }
}

/// Write the config, creating the directory if needed.
///
/// Writes to a sibling temporary file and renames, so an interrupted save
/// cannot leave the user with a half-written keymap and no way to fix it from
/// the UI that reads it.
pub fn save(config: &KeybindingsConfig) -> std::io::Result<PathBuf> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not determine config directory",
        )
    })?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let serialized = serde_json::to_string_pretty(config)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serialized.as_bytes())?;
    std::fs::rename(&temp, &path)?;
    Ok(path)
}

fn plain(message: String) -> Diagnostic {
    Diagnostic {
        message,
        line: None,
        column: None,
    }
}

fn response(
    config: KeybindingsConfig,
    diagnostics: Vec<Diagnostic>,
    path: Option<String>,
) -> KeybindingsResponse {
    KeybindingsResponse {
        config,
        diagnostics,
        path,
    }
}
