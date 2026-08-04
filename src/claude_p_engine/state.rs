//! Registry, session mutators, pending-hook plumbing, and `claude -p` turn options.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use super::{now_ms, rand_id, ClaudePEngine, Session, SessionsGet};
use crate::claude_engine::{claude_cli, PendingReply};

impl ClaudePEngine {
    // ── registry lookups ─────────────────────────────────────────────
    pub fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.lock().ok()?.get(id).cloned()
    }

    pub fn list_for_dir(&self, dir: &str) -> Vec<Session> {
        let mut v: Vec<Session> = self
            .sessions
            .lock()
            .map(|s| s.values().filter(|x| x.directory == dir).cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| b.created.cmp(&a.created));
        v
    }

    /// session id → busy, for `GET /session/status`.
    pub fn busy_map(&self) -> HashMap<String, bool> {
        self.sessions
            .lock()
            .map(|s| s.values().map(|x| (x.id.clone(), x.busy)).collect())
            .unwrap_or_default()
    }

    pub fn session_id_for_claude_uuid(&self, uuid: &str) -> Option<String> {
        if uuid.is_empty() {
            return None;
        }
        let s = self.sessions.lock().ok()?;
        s.values()
            .find(|x| x.claude_uuid.as_deref() == Some(uuid))
            .map(|x| x.id.clone())
    }

    /// The claude UUID to `--resume` for this session, if a prior turn established one.
    pub fn resume_uuid(&self, id: &str) -> Option<String> {
        self.get_session(id).and_then(|s| s.claude_uuid)
    }

    // ── session lifecycle ────────────────────────────────────────────
    pub fn create_session(&self, dir: &str, parent_id: &str, title: &str) -> Session {
        let now = now_ms();
        let entry = Session {
            id: rand_id("ses"),
            title: title.to_string(),
            directory: dir.to_string(),
            parent_id: parent_id.to_string(),
            created: now,
            updated: now,
            ..Default::default()
        };
        if let Ok(mut s) = self.sessions.lock() {
            s.insert(entry.id.clone(), entry.clone());
        }
        self.save();
        self.emit(
            dir,
            "session.created",
            json!({ "info": super::session_info(&entry) }),
        );
        entry
    }

    /// Register a child row for a claude subagent (`Task`) so it nests under its parent
    /// in the sidebar. The session `id` is the claude `agentId`. Idempotent; emits
    /// `session.created` only on first registration.
    pub fn ensure_subagent_session(&self, parent_id: &str, agent_id: &str, title: &str, dir: &str) {
        if agent_id.is_empty() {
            return;
        }
        let created = {
            let mut g = match self.sessions.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if g.contains_key(agent_id) {
                None
            } else {
                let now = now_ms();
                let entry = Session {
                    id: agent_id.to_string(),
                    title: if title.is_empty() {
                        "Subagent".to_string()
                    } else {
                        title.to_string()
                    },
                    directory: dir.to_string(),
                    parent_id: parent_id.to_string(),
                    created: now,
                    updated: now,
                    is_subagent: true,
                    ..Default::default()
                };
                g.insert(agent_id.to_string(), entry.clone());
                Some(entry)
            }
        };
        if let Some(entry) = created {
            self.emit(
                dir,
                "session.created",
                json!({ "info": super::session_info(&entry) }),
            );
        }
    }

    pub fn rename_session(&self, id: &str, title: &str) {
        self.set_title(id, title, true);
    }

    pub async fn delete_session(self: &Arc<Self>, id: &str) {
        super::process::abort(self.clone(), id).await;
        let dir = self
            .sessions
            .lock()
            .ok()
            .and_then(|mut s| s.remove(id))
            .map(|e| e.directory);
        self.save();
        if let Some(dir) = dir {
            self.emit(
                &dir,
                "session.deleted",
                json!({ "sessionID": id, "id": id }),
            );
        }
    }

    pub fn set_title(&self, id: &str, title: &str, manual: bool) {
        let changed = {
            let mut g = match self.sessions.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(e) = g.get_mut(id) else { return };
            if (!manual && e.title_locked) || e.title == title {
                false
            } else {
                e.title = title.to_string();
                if manual {
                    e.title_locked = true;
                }
                e.updated = now_ms();
                true
            }
        };
        if !changed {
            return;
        }
        self.save();
        if let Some(e) = self.get_session(id) {
            self.emit(
                &e.directory,
                "session.updated",
                json!({ "info": super::session_info(&e) }),
            );
        }
    }

    pub fn set_claude_uuid(&self, id: &str, uuid: &str) {
        let changed = self
            .get_session(id)
            .map(|s| s.claude_uuid.as_deref() != Some(uuid))
            .unwrap_or(false);
        if !changed {
            return;
        }
        self.mutate(id, |e| e.claude_uuid = Some(uuid.to_string()));
        self.save();
    }

    /// Forget a session's resume UUID (e.g. a `--resume` died as stale), so the next
    /// message starts a fresh conversation rather than looping on a bad resume.
    pub fn forget_claude_uuid(&self, id: &str) {
        self.mutate(id, |e| e.claude_uuid = None);
        self.save();
    }

    pub fn set_model(&self, id: &str, model: &str) {
        if model.is_empty() {
            return;
        }
        self.mutate(id, |e| e.model = Some(model.to_string()));
        self.save();
    }

    pub fn set_agent(&self, id: &str, agent: &str) {
        let resolved = self.resolve_agent(id, agent);
        self.mutate(id, |e| {
            e.agent = (!resolved.is_empty()).then(|| resolved.clone())
        });
        self.save();
    }

    pub fn set_permission_mode(&self, id: &str, mode: &str) {
        let dir = self.mutate(id, |e| e.permission_mode = Some(mode.to_string()));
        self.save();
        if let Some(dir) = dir {
            self.emit(
                &dir,
                "tui.toast.show",
                json!({ "message": format!("Claude permission mode: {mode}"), "variant": "info" }),
            );
        }
    }

    /// Set busy + emit status; returns true on a busy→idle edge.
    pub fn set_busy(&self, id: &str, busy: bool) -> bool {
        let (dir, changed) = {
            let mut g = match self.sessions.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };
            let Some(e) = g.sessions_get_mut(id) else {
                return false;
            };
            let changed = e.busy != busy;
            e.busy = busy;
            (e.directory.clone(), changed)
        };
        if !changed {
            return false;
        }
        let status = if busy { "busy" } else { "idle" };
        self.emit(
            &dir,
            "session.status",
            json!({ "sessionID": id, "status": { "type": status } }),
        );
        if !busy {
            self.emit(&dir, "session.idle", json!({ "sessionID": id }));
        }
        !busy
    }

    pub fn effective_mode(&self, id: &str) -> String {
        self.get_session(id)
            .and_then(|s| s.permission_mode)
            .unwrap_or_else(|| self.default_mode.clone())
    }

    pub fn add_allowed_tool(&self, id: &str, tool: &str) {
        self.mutate(id, |e| {
            if !e.allowed_tools.iter().any(|t| t == tool) {
                e.allowed_tools.push(tool.to_string());
            }
        });
        self.save();
    }

    pub fn is_always_allowed(&self, id: &str, tool: &str) -> bool {
        self.get_session(id)
            .map(|s| s.allowed_tools.iter().any(|t| t == tool))
            .unwrap_or(false)
    }

    // ── pending hook requests ────────────────────────────────────────
    pub fn register_pending(&self, id: &str) -> tokio::sync::oneshot::Receiver<PendingReply> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id.to_string(), tx);
        }
        rx
    }

    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let tx = self.pending.lock().ok().and_then(|mut p| p.remove(id));
        tx.map(|tx| tx.send(reply).is_ok()).unwrap_or(false)
    }

    // ── init introspection cache ─────────────────────────────────────
    pub fn cached_init(&self, dir: &str) -> Option<claude_cli::InitInfo> {
        self.command_cache.lock().ok()?.get(dir).cloned()
    }

    pub fn set_cached_init(&self, dir: &str, info: claude_cli::InitInfo) {
        if let Ok(mut c) = self.command_cache.lock() {
            c.insert(dir.to_string(), info);
        }
    }

    pub(super) fn resolve_agent(&self, id: &str, agent: &str) -> String {
        let agent = agent.trim();
        if agent.is_empty() {
            return String::new();
        }
        let dir = self
            .get_session(id)
            .map(|s| s.directory)
            .unwrap_or_default();
        let known = self.cached_init(&dir).map(|i| i.agents).unwrap_or_default();
        let find = |name: &str| known.iter().find(|a| a.eq_ignore_ascii_case(name)).cloned();
        if let Some(real) = find(agent) {
            return real;
        }
        match agent.to_ascii_lowercase().as_str() {
            "plan" => find("Plan").unwrap_or_else(|| {
                if known.is_empty() {
                    "Plan".to_string()
                } else {
                    String::new()
                }
            }),
            "build" | "code-reviewer" | "reviewer" => String::new(),
            _ => {
                if known.is_empty() {
                    agent.to_string()
                } else {
                    String::new()
                }
            }
        }
    }

    // ── helpers ──────────────────────────────────────────────────────
    fn mutate(&self, id: &str, f: impl FnOnce(&mut Session)) -> Option<String> {
        let mut g = self.sessions.lock().ok()?;
        let e = g.get_mut(id)?;
        f(e);
        Some(e.directory.clone())
    }

    fn save(&self) {
        if let Ok(g) = self.sessions.lock() {
            super::session::save_sessions(&self.persist, &g);
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

#[cfg(test)]
#[path = "state_poison_tests.rs"]
mod state_poison_tests;
