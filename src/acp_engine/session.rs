//! Session model and disk persistence for the ACP engine.
//!
//! Persisted state is deliberately small: titles, the user's engine choices, and the
//! agent's session id. Everything else — the transcript, tool state, todos — is rebuilt by
//! replaying `session/load`, so opman never has to keep a second copy of the conversation
//! in sync with the agent's.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One opman session backed by an ACP server.
#[derive(Clone, Default)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub parent_id: String,
    pub created: u64,
    pub updated: u64,
    /// The agent's session id, replayed via `session/load` to continue after a restart.
    pub acp_session: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub effort: Option<String>,
    /// Permission mode, expressed as the agent's own mode id.
    pub permission_mode: Option<String>,
    pub allowed_tools: Vec<String>,
    pub busy: bool,
    pub title_locked: bool,
    /// A child row synthesized from a subagent launch; its transcript is read from disk
    /// rather than streamed, since ACP has no child sessions.
    pub is_subagent: bool,
}

/// opencode-shaped session object for REST responses and events.
///
/// `engine` carries the choices this session is configured with. They are already on the
/// `Session` above and already on disk, so reporting them costs nothing — and without them
/// the composer has no way to know what a session runs as and falls back to whatever the
/// last session touched happened to use.
pub fn session_info(s: &Session) -> Value {
    json!({
        "id": s.id,
        "slug": "",
        "title": s.title,
        "version": env!("CARGO_PKG_VERSION"),
        "projectID": "acp",
        "parentID": s.parent_id,
        "directory": s.directory,
        "time": { "created": s.created, "updated": s.updated },
        "engine": engine_choices(s),
    })
}

/// The engine choices, with the agent's own configured defaults standing in for anything
/// the user has never picked — that is what the next turn will actually run as.
fn engine_choices(s: &Session) -> crate::app::EngineChoices {
    crate::app::EngineChoices::from_parts(
        s.model.as_deref(),
        s.agent.as_deref(),
        s.effort.as_deref(),
        s.permission_mode.as_deref(),
    )
}

#[derive(Serialize, Deserialize)]
struct PersistSession {
    id: String,
    title: String,
    directory: String,
    parent_id: String,
    created: u64,
    updated: u64,
    #[serde(default)]
    acp_session: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    title_locked: bool,
}

impl From<&Session> for PersistSession {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            directory: s.directory.clone(),
            parent_id: s.parent_id.clone(),
            created: s.created,
            updated: s.updated,
            acp_session: s.acp_session.clone(),
            model: s.model.clone(),
            agent: s.agent.clone(),
            effort: s.effort.clone(),
            permission_mode: s.permission_mode.clone(),
            title_locked: s.title_locked,
        }
    }
}

/// Write the registry to disk (best-effort). Subagent rows are transient.
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
    if let Ok(serialized) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write(path, serialized);
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
                    acp_session: p.acp_session,
                    model: p.model,
                    agent: p.agent,
                    effort: p.effort,
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
