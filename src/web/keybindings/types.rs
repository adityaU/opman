//! Serde types for the web UI keybinding config.
//!
//! The backend deliberately does not interpret chords or command ids: the
//! command registry lives in the web UI, so only it can say whether
//! `session.new` exists or whether `ctrl+k ctrl+w` parses. What this layer
//! guarantees is narrower and still worth having — the file is valid JSON, has
//! the right shape, and is written atomically.

use serde::{Deserialize, Serialize};

/// Which keymap the user is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Normal,
    Vim,
}

/// Which-key hint behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WhichKey {
    pub enabled: bool,
    pub delay_ms: u32,
    pub sort_by: SortBy,
}

impl Default for WhichKey {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 400,
            sort_by: SortBy::Group,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    #[default]
    Group,
    Key,
    Label,
}

/// One authored binding. Mirrors `BindingSpec` in the web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub key: String,
    /// A leading `-` removes an earlier binding instead of adding one.
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The whole file. Every field defaults, so an empty `{}` is valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KeybindingsConfig {
    pub mode: Mode,
    pub leader: String,
    pub local_leader: String,
    pub chord_timeout_ms: u32,
    pub which_key: WhichKey,
    pub bindings: Vec<Binding>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            leader: "<space>".to_string(),
            local_leader: ",".to_string(),
            chord_timeout_ms: 1500,
            which_key: WhichKey::default(),
            bindings: Vec::new(),
        }
    }
}

/// A problem found while loading. Reported rather than raised, so one bad file
/// never costs the user their whole keymap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

/// What `GET /api/keybindings` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeybindingsResponse {
    pub config: KeybindingsConfig,
    pub diagnostics: Vec<Diagnostic>,
    /// Absolute path, so the view can show "Open keybindings.json".
    pub path: Option<String>,
}
