//! What the user chose for "a runner is running low", kept in
//! `~/.config/opman/runner-limits.json`.
//!
//! The server owns these rather than the browser because the switch has to happen with no
//! browser open: a session left working in the background — a kanban task, an agent the
//! agent-manager started — must move to another runner on its own when its plan runs out.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// What happens when the runner a session uses gets low.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitPolicy {
    /// The composer offers another runner; nothing moves until the user picks it.
    #[default]
    Ask,
    /// opman moves the session itself, to the first pair of the user's list with room.
    Auto,
    /// No warnings, no switching.
    Off,
}

/// Percent left at or under which a runner counts as low. Always 1–99: 0 would never
/// fire and 100 would always fire, so neither is a threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct NearPercent(u8);

impl NearPercent {
    pub const DEFAULT: Self = Self(5);

    pub fn get(self) -> f64 {
        f64::from(self.0)
    }
}

impl Default for NearPercent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u8> for NearPercent {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=99).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!("nearPercent must be 1–99, got {value}"))
        }
    }
}

impl From<NearPercent> for u8 {
    fn from(value: NearPercent) -> Self {
        value.0
    }
}

/// A model inside one runner's catalogue, spelled the way the send body spells it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// One place a session may move to. `model: None` is the runner's own default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackPair {
    pub runner: String,
    #[serde(default)]
    pub model: Option<ModelRef>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LimitSettings {
    pub policy: LimitPolicy,
    pub near_percent: NearPercent,
    /// Tried top to bottom.
    pub pairs: Vec<FallbackPair>,
}

/// The settings file and the copy every reader shares. A read is an `Arc` clone under a
/// short lock; a write replaces the whole value, so readers never see half an edit.
pub struct SettingsStore {
    path: Option<PathBuf>,
    current: RwLock<Arc<LimitSettings>>,
}

impl SettingsStore {
    /// Read `path`, falling back to defaults when it is missing or unreadable. A broken
    /// file must not stop opman from starting; the next save overwrites it.
    pub fn open(path: Option<PathBuf>) -> Self {
        let current = path.as_deref().and_then(read).unwrap_or_default();
        Self {
            path,
            current: RwLock::new(Arc::new(current)),
        }
    }

    pub fn get(&self) -> Arc<LimitSettings> {
        let guard = self.current.read().unwrap_or_else(|poison| poison.into_inner());
        Arc::clone(&guard)
    }

    /// Persist then publish, so what readers see is always what is on disk.
    pub fn set(&self, next: LimitSettings) -> Result<Arc<LimitSettings>> {
        if let Some(path) = &self.path {
            write(path, &next)?;
        }
        let next = Arc::new(next);
        let mut guard = self.current.write().unwrap_or_else(|poison| poison.into_inner());
        *guard = Arc::clone(&next);
        Ok(next)
    }
}

fn read(path: &Path) -> Option<LimitSettings> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&raw) {
        Ok(settings) => Some(settings),
        Err(error) => {
            tracing::warn!("ignoring {}: {error}", path.display());
            None
        }
    }
}

/// Write through a temp file and a rename, so a crash mid-save leaves the old file.
fn write(path: &Path, settings: &LimitSettings) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(settings)?;
    std::fs::write(&tmp, json).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

/// The process-wide store, at `~/.config/opman/runner-limits.json`.
pub fn global() -> &'static SettingsStore {
    static STORE: LazyLock<SettingsStore> = LazyLock::new(|| {
        SettingsStore::open(dirs::config_dir().map(|d| d.join("opman").join("runner-limits.json")))
    });
    &STORE
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
