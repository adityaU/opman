//! `~/.config/opman/acp.json` — the user's half of the agent registry.
//!
//! This is a deliberately different type from the resolved [`AgentConfig`] the engine
//! reads. On disk every field is optional, and `None` means "the built-in decides" while
//! `Some` means "the user decided, including deciding on nothing". Collapsing the two into
//! one struct — the shape this module replaced — forced the merge to guess, by treating
//! "differs from the default" as "was written down". That guess is unrepresentable here,
//! which is the point: clearing an agent's arguments back to empty is a `Some(vec![])`, not
//! an absence indistinguishable from silence.
//!
//! Removing an entry therefore never deletes an agent opman ships — it restores it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::config::{AgentConfig, ClientCaps};

/// One agent as the user wrote it. Every field is an override; absent means untouched.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_remove: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_caps: Option<ClientCaps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inject_mcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes_are_agents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_transcripts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Move a set value into its slot, leaving the target alone when the user said nothing.
fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

impl AgentPatch {
    /// Overlay this entry onto a resolved config. Consumes the patch so owned strings and
    /// vectors move into place rather than being cloned.
    pub fn apply(self, target: &mut AgentConfig) {
        set(&mut target.display_name, self.display_name);
        set(&mut target.command, self.command);
        set(&mut target.args, self.args);
        set(&mut target.env, self.env);
        set(&mut target.env_remove, self.env_remove);
        set(&mut target.runner, self.runner);
        set(&mut target.client_caps, self.client_caps);
        set(&mut target.inject_mcp, self.inject_mcp);
        set(&mut target.default_mode, self.default_mode);
        set(&mut target.default_model, self.default_model);
        set(&mut target.modes_are_agents, self.modes_are_agents);
        set(&mut target.subagent_transcripts, self.subagent_transcripts);
        set(&mut target.enabled, self.enabled);
    }

    /// True when this entry overrides nothing, so writing it would only add noise.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// The whole `acp.json` document.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AcpDocument {
    pub agents: BTreeMap<String, AgentPatch>,
}

/// Path of the user config file. `$OPMAN_ACP_CONFIG` wins when set.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("OPMAN_ACP_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    dirs::config_dir().map(|dir| dir.join("opman").join("acp.json"))
}

/// Read the user's file. A missing file is an empty document; a malformed one warns and is
/// ignored, so a typo cannot stop opman from starting with its built-in agents.
pub fn load_document() -> AcpDocument {
    let Some(path) = config_path() else {
        return AcpDocument::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return AcpDocument::default();
    };
    match serde_json::from_str(&raw) {
        Ok(document) => document,
        Err(error) => {
            tracing::warn!(path = %path.display(), "ignoring malformed acp.json: {error}");
            AcpDocument::default()
        }
    }
}

/// Write the document back, creating `~/.config/opman` if this is the first edit.
pub fn save_document(document: &AcpDocument) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(document)?)?;
    Ok(())
}

/// Delete the file outright, returning whether there was one to delete.
///
/// Not the same as writing an empty document: an absent file is the state a fresh install
/// is in, so this is what "reset every agent to how opman ships it" means. A missing file
/// is success, because the caller asked for it to be gone and it is.
pub fn delete_document() -> anyhow::Result<bool> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
#[path = "patch_tests.rs"]
mod patch_tests;
