//! `~/.config/opman/runners.json` — which built-in runners are switched off.
//!
//! ```json
//! { "disabled": ["opencode"], "default": "claude-code" }
//! ```
//!
//! A file of its own, next to `acp.json` and `mcp.json`, for the same reason those are
//! separate: it is edited from one settings section and read at one point in startup, and
//! keeping it apart means a malformed edit here cannot cost the user their agent config.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::BuiltinRunner;

/// The decisions the file records.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunnersDocument {
    /// Built-in runners the user switched off.
    pub disabled: BTreeSet<BuiltinRunner>,
    /// Where a new session goes when none is named — written when switching the default
    /// off forced a new one. A runner label, so it may name an ACP agent.
    pub default: Option<String>,
}

/// The on-disk spelling. Names are kept as strings so one opman does not recognise — a
/// newer version's runner, a typo — is skipped rather than failing the whole file.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Raw {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    disabled: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
}

impl RunnersDocument {
    pub fn is_disabled(&self, runner: BuiltinRunner) -> bool {
        self.disabled.contains(&runner)
    }

    fn from_raw(raw: Raw) -> Self {
        let disabled = raw
            .disabled
            .iter()
            .filter_map(|name| {
                let parsed = BuiltinRunner::parse(name);
                if parsed.is_none() {
                    tracing::warn!(runner = %name, "runners.json: ignoring unknown runner");
                }
                parsed
            })
            .collect();
        let default = raw
            .default
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty());
        Self { disabled, default }
    }

    fn to_raw(&self) -> Raw {
        Raw {
            disabled: self
                .disabled
                .iter()
                .map(|runner| runner.id().to_string())
                .collect(),
            default: self.default.clone(),
        }
    }
}

/// Where the file lives. `$OPMAN_RUNNERS_CONFIG` wins when set.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("OPMAN_RUNNERS_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    dirs::config_dir().map(|dir| dir.join("opman").join("runners.json"))
}

/// Read the file. Missing means "everything on"; malformed warns and means the same, so a
/// bad edit can never leave opman with no runner to start.
pub fn load(path: &Path) -> RunnersDocument {
    let Ok(text) = std::fs::read_to_string(path) else {
        return RunnersDocument::default();
    };
    match serde_json::from_str::<Raw>(&text) {
        Ok(raw) => RunnersDocument::from_raw(raw),
        Err(error) => {
            tracing::warn!(path = %path.display(), "ignoring malformed runners.json: {error}");
            RunnersDocument::default()
        }
    }
}

/// Write the file, creating `~/.config/opman` on the first edit.
pub fn save(path: &Path, document: &RunnersDocument) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&document.to_raw())?;
    std::fs::write(path, text)?;
    Ok(())
}
