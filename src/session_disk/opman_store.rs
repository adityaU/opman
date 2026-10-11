//! The session registries opman persists for its own engines.
//!
//! The embedded claude engine writes `{ "sessions": { id: entry }, "deleted": [...] }`; an ACP
//! engine writes a bare array of entries. Both entries share the field names read here, and
//! both engines list a directory's sessions newest-created first, so this does the same.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

use crate::app::{EngineChoices, SessionInfo, SessionTime};

/// The part of a persisted entry a sidebar row needs.
#[derive(Deserialize)]
struct StoredSession {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    directory: String,
    #[serde(default)]
    parent_id: String,
    #[serde(default)]
    created: u64,
    #[serde(default)]
    updated: u64,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
}

impl From<StoredSession> for SessionInfo {
    fn from(stored: StoredSession) -> Self {
        let engine = EngineChoices::from_parts(
            stored.model.as_deref(),
            stored.agent.as_deref(),
            stored.effort.as_deref(),
            stored.permission_mode.as_deref(),
        );
        Self {
            id: stored.id,
            title: stored.title,
            parent_id: stored.parent_id,
            directory: stored.directory,
            time: SessionTime {
                created: stored.created,
                updated: stored.updated,
            },
            engine,
        }
    }
}

#[derive(Deserialize)]
struct ClaudeRegistry {
    #[serde(default)]
    sessions: HashMap<String, StoredSession>,
}

/// Sessions of the embedded claude engine for `directory`.
pub(super) fn list_claude_registry(
    path: &Path,
    directory: &str,
) -> anyhow::Result<Vec<SessionInfo>> {
    let Some(raw) = read_if_present(path)? else {
        return Ok(Vec::new());
    };
    let registry: ClaudeRegistry =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(for_directory(registry.sessions.into_values(), directory))
}

/// Sessions of one ACP agent for `directory`.
pub(super) fn list_acp_registry(path: &Path, directory: &str) -> anyhow::Result<Vec<SessionInfo>> {
    let Some(raw) = read_if_present(path)? else {
        return Ok(Vec::new());
    };
    let entries: Vec<StoredSession> =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(for_directory(entries, directory))
}

fn for_directory(
    entries: impl IntoIterator<Item = StoredSession>,
    directory: &str,
) -> Vec<SessionInfo> {
    let mut found: Vec<SessionInfo> = entries
        .into_iter()
        .filter(|entry| entry.directory == directory)
        .map(SessionInfo::from)
        .collect();
    found.sort_by(|a, b| b.time.created.cmp(&a.time.created));
    found
}

/// The file's contents, or `None` when the engine has never written it.
fn read_if_present(path: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(Some(raw)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

#[cfg(test)]
#[path = "opman_store_tests.rs"]
mod opman_store_tests;
