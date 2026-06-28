//! Session registry for the embedded Claude engine.
//!
//! opman expects stable, opaque session ids (it keys PTYs, web state, and Slack
//! threads on them). The `claude` CLI, by contrast, mints a *new* session UUID on
//! every `--bg`/`--bg --resume` turn. The registry bridges the two: opman only ever
//! sees an opencode-style id (`ses_…`), and we map it to the *latest* claude session
//! UUID + background short-id.
//!
//! `claude --bg --resume <uuid>` writes a fresh transcript containing the **full**
//! conversation history, so the latest claude session UUID alone is enough to render
//! the entire conversation — no concatenation across the lineage is required.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One logical opman session, backed by a chain of claude background sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionEntry {
    /// opencode-style id exposed to opman (`ses_…`).
    pub id: String,
    /// Human-readable title (seeded from the prompt, refined by claude's `ai-title`).
    pub title: String,
    /// Absolute project directory this session runs in.
    pub directory: String,
    /// Parent opman session id (empty for root sessions).
    pub parent_id: String,
    /// Creation time (epoch millis).
    pub created: u64,
    /// Last-updated time (epoch millis).
    pub updated: u64,
    /// Current claude background short id (for `claude attach`/`stop`/`logs`). None
    /// until the first prompt spawns a background agent.
    pub short_id: Option<String>,
    /// Latest claude session UUID (for `--resume` and locating the transcript JSONL).
    pub claude_session_id: Option<String>,
    /// All claude session UUIDs in order (newest last). Useful for cleanup/debugging.
    #[serde(default)]
    pub lineage: Vec<String>,
    /// Model alias/name to pass to claude (`sonnet`, `opus`, …). None = claude default.
    #[serde(default)]
    pub model: Option<String>,
    /// Per-session permission mode override (`default`, `acceptEdits`,
    /// `bypassPermissions`, `plan`, …). None = engine default. Changeable at runtime.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Tools the user chose to "always allow" for this session.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Whether the backing agent is currently working.
    #[serde(default, skip)]
    pub busy: bool,
}

/// In-memory map of opman session id → entry, with best-effort disk persistence.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub sessions: HashMap<String, SessionEntry>,
}

impl Registry {
    pub fn load(path: &PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Registry::default(),
        }
    }

    pub fn save(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }

    /// Sessions belonging to a given project directory, newest first.
    pub fn for_directory(&self, dir: &str) -> Vec<SessionEntry> {
        let mut v: Vec<SessionEntry> = self
            .sessions
            .values()
            .filter(|s| s.directory == dir)
            .cloned()
            .collect();
        v.sort_by(|a, b| b.created.cmp(&a.created));
        v
    }

    /// Find the entry whose latest claude session UUID matches `uuid`.
    #[allow(dead_code)] // paired with ClaudeEngine::session_id_for_claude_uuid
    pub fn by_claude_uuid(&self, uuid: &str) -> Option<&SessionEntry> {
        self.sessions
            .values()
            .find(|s| s.claude_session_id.as_deref() == Some(uuid))
    }
}
