//! Session model + disk persistence for the `claude -p` engine.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::claude_engine::claude_cli;

/// One opman session backed by a live `claude -p` process.
#[derive(Clone, Default)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub parent_id: String,
    pub created: u64,
    pub updated: u64,
    /// claude session UUID (from the process's `system/init`). Locates the on-disk
    /// transcript for rendering and is replayed via `--resume` to continue the
    /// conversation across abort/restart. Persisted.
    pub claude_uuid: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub permission_mode: Option<String>,
    pub allowed_tools: Vec<String>,
    pub busy: bool,
    pub title_locked: bool,
    /// True for a child row synthesized from a claude subagent (`Task`); its `id` is the
    /// claude `agentId` and its transcript is located via `locate_subagent_jsonl`.
    pub is_subagent: bool,
}

/// opencode-shaped session object for REST responses + events.
pub fn session_info(s: &Session) -> Value {
    json!({
        "id": s.id,
        "slug": "",
        "title": s.title,
        "version": claude_cli::version(),
        "projectID": "claude",
        "parentID": s.parent_id,
        "directory": s.directory,
        "time": { "created": s.created, "updated": s.updated },
    })
}

/// Persisted subset (titles/config + claude UUID for resume; live process state is not).
#[derive(Serialize, Deserialize)]
struct PersistSession {
    id: String,
    title: String,
    directory: String,
    parent_id: String,
    created: u64,
    updated: u64,
    #[serde(default)]
    claude_uuid: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    title_locked: bool,
}

impl From<&Session> for PersistSession {
    fn from(s: &Session) -> Self {
        PersistSession {
            id: s.id.clone(),
            title: s.title.clone(),
            directory: s.directory.clone(),
            parent_id: s.parent_id.clone(),
            created: s.created,
            updated: s.updated,
            claude_uuid: s.claude_uuid.clone(),
            model: s.model.clone(),
            agent: s.agent.clone(),
            permission_mode: s.permission_mode.clone(),
            title_locked: s.title_locked,
        }
    }
}

/// Serialize the registry to disk (best-effort). Subagent rows are transient (rebuilt
/// from the parent transcript) and are not persisted.
pub fn save_sessions(persist: &Option<PathBuf>, sessions: &HashMap<String, Session>) {
    let Some(path) = persist else { return };
    let snapshot: Vec<PersistSession> = sessions
        .values()
        .filter(|s| !s.is_subagent)
        .map(PersistSession::from)
        .collect();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write(path, s);
    }
}

pub fn load_sessions(persist: &Option<PathBuf>) -> HashMap<String, Session> {
    let Some(path) = persist else {
        return HashMap::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let list: Vec<PersistSession> = serde_json::from_str(&raw).unwrap_or_default();
    list.into_iter()
        .map(|p| {
            (
                p.id.clone(),
                Session {
                    id: p.id,
                    title: p.title,
                    directory: p.directory,
                    parent_id: p.parent_id,
                    created: p.created,
                    updated: p.updated,
                    claude_uuid: p.claude_uuid,
                    model: p.model,
                    agent: p.agent,
                    permission_mode: p.permission_mode,
                    allowed_tools: Vec::new(),
                    busy: false,
                    title_locked: p.title_locked,
                    is_subagent: false,
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;
