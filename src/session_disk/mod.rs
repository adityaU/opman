//! Session lists read straight from each runner's own on-disk store.
//!
//! Runners start on first use, so at boot nothing is running and a runner that is not up
//! cannot be asked for its sessions. Every runner already keeps those sessions on disk,
//! though, and listing them needs only a little metadata — id, title, directory, times — not
//! the transcript. These readers open that store read-only, so the sidebar can show a
//! project's previous sessions without spawning a single process. Opening one still starts
//! its runner, which is what renders the conversation.
//!
//! Each reader returns exactly what its runner would list once it is up, so the live list
//! that replaces the disk one later neither drops nor adds rows:
//! - opencode: its SQLite database (or the JSON layout older releases used), capped at the
//!   100 most recently updated sessions the way opencode's own `GET /session` is.
//! - the embedded claude engine and every ACP agent: the registry opman itself persists for
//!   that engine. Their sessions are opman-owned ids bound to the agent's own session ids,
//!   so the agents' stores (`~/.claude/projects`, `~/.codex/sessions`) are not opman
//!   sessions at all — listing them would show rows the runner cannot open.

mod opencode_db;
mod opencode_json;
mod opman_store;

use std::path::{Path, PathBuf};

use crate::app::SessionInfo;

/// How many sessions opencode's `GET /session` returns when no limit is given.
const OPENCODE_LIST_LIMIT: usize = 100;

/// Where one runner keeps its sessions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStore {
    /// opencode's data directory (`$XDG_DATA_HOME/opencode`).
    Opencode(PathBuf),
    /// The registry file of the embedded claude engine (`claude_sessions.json`).
    ClaudeEngine(PathBuf),
    /// The registry file of one ACP agent (`acp_<id>_sessions.json`).
    Acp(PathBuf),
}

impl SessionStore {
    /// opencode's store. opencode resolves it with `xdg-basedir` on every platform, so it is
    /// `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode` — never the OS data dir.
    pub fn opencode() -> Option<Self> {
        let base = match std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            Some(data) => PathBuf::from(data),
            None => dirs::home_dir()?.join(".local").join("share"),
        };
        Some(Self::Opencode(base.join("opencode")))
    }

    /// The embedded claude engine's registry — the path `claude_engine` persists to.
    pub fn claude_engine() -> Option<Self> {
        opman_config_file("claude_sessions.json").map(Self::ClaudeEngine)
    }

    /// One ACP agent's registry — the path `acp_engine` persists that agent to.
    pub fn acp(agent_id: &str) -> Option<Self> {
        opman_config_file(&format!("acp_{agent_id}_sessions.json")).map(Self::Acp)
    }

    /// The store behind the default runner slot, chosen by the backend that serves it.
    pub fn for_backend(backend: crate::cli::AgentBackend, acp_agent: Option<&str>) -> Option<Self> {
        match backend {
            crate::cli::AgentBackend::Opencode => Self::opencode(),
            crate::cli::AgentBackend::ClaudeCode => Self::claude_engine(),
            crate::cli::AgentBackend::ClaudeAcp => acp_agent.and_then(Self::acp),
        }
    }

    /// Sessions recorded for `directory`. Blocking file IO — call through [`Self::list_async`]
    /// from async code. A store that does not exist yet is simply empty; one that cannot be
    /// read is logged and treated as empty, since a sidebar row is never worth a failed boot.
    pub fn list(&self, directory: &str) -> Vec<SessionInfo> {
        let listed = match self {
            Self::Opencode(root) => list_opencode(root, directory),
            Self::ClaudeEngine(path) => opman_store::list_claude_registry(path, directory),
            Self::Acp(path) => opman_store::list_acp_registry(path, directory),
        };
        listed.unwrap_or_else(|error| {
            tracing::warn!(store = ?self, %directory, "could not list sessions from disk: {error:#}");
            Vec::new()
        })
    }

    /// [`Self::list`] on the blocking pool.
    pub async fn list_async(&self, directory: &str) -> Vec<SessionInfo> {
        let store = self.clone();
        let directory = directory.to_string();
        tokio::task::spawn_blocking(move || store.list(&directory))
            .await
            .unwrap_or_default()
    }
}

/// A file in opman's config directory, where both native engines persist their sessions.
fn opman_config_file(name: &str) -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("opman").join(name))
}

/// opencode moved from one JSON file per session to SQLite; read whichever is present.
fn list_opencode(root: &Path, directory: &str) -> anyhow::Result<Vec<SessionInfo>> {
    match opencode_db::database(root) {
        Some(db) => opencode_db::list(&db, directory, OPENCODE_LIST_LIMIT),
        None => opencode_json::list(&root.join("storage"), directory, OPENCODE_LIST_LIMIT),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
