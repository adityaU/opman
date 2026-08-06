//! The durable session registry: lookups, lifecycle, and the engine choices a user makes.
//!
//! Per-turn bookkeeping lives in [`super::turn_state`] and agent-reported capability in
//! [`super::discovered`]; both are rebuilt rather than persisted.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use super::{now_ms, rand_id, AcpEngine, Discovered, Session};
use crate::claude_engine::PendingReply;

impl AcpEngine {
    // ── registry ─────────────────────────────────────────────────────
    pub fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.lock().ok()?.get(id).cloned()
    }

    pub fn list_for_dir(&self, dir: &str) -> Vec<Session> {
        let mut found: Vec<Session> = self
            .sessions
            .lock()
            .map(|s| s.values().filter(|x| x.directory == dir).cloned().collect())
            .unwrap_or_default();
        found.sort_by(|a, b| b.created.cmp(&a.created));
        found
    }

    pub fn busy_map(&self) -> HashMap<String, bool> {
        self.sessions
            .lock()
            .map(|s| s.values().map(|x| (x.id.clone(), x.busy)).collect())
            .unwrap_or_default()
    }

    pub fn is_busy(&self, id: &str) -> bool {
        self.get_session(id).map(|s| s.busy).unwrap_or(false)
    }

    pub fn url(&self) -> String {
        self.url.lock().map(|u| u.clone()).unwrap_or_default()
    }

    pub(super) fn set_url(&self, url: &str) {
        if let Ok(mut current) = self.url.lock() {
            *current = url.to_string();
        }
    }

    // ── lifecycle ────────────────────────────────────────────────────
    pub fn create_session(&self, dir: &str, parent_id: &str, title: &str) -> Session {
        let now = now_ms();
        let entry = Session {
            id: rand_id("ses"),
            title: title.to_string(),
            directory: dir.to_string(),
            parent_id: parent_id.to_string(),
            created: now,
            updated: now,
            permission_mode: (!self.agent.default_mode.is_empty())
                .then(|| self.agent.default_mode.clone()),
            model: (!self.agent.default_model.is_empty()).then(|| self.agent.default_model.clone()),
            ..Default::default()
        };
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(entry.id.clone(), entry.clone());
        }
        self.save();
        self.emit(
            dir,
            "session.created",
            json!({ "info": super::session_info(&entry) }),
        );
        entry
    }

    /// Register a child row for a subagent launch so it nests under its parent. Idempotent.
    pub fn ensure_subagent_session(&self, parent_id: &str, agent_id: &str, title: &str, dir: &str) {
        if agent_id.is_empty() {
            return;
        }
        let created = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return;
            };
            if sessions.contains_key(agent_id) {
                None
            } else {
                let now = now_ms();
                let entry = Session {
                    id: agent_id.to_string(),
                    title: if title.is_empty() { "Subagent" } else { title }.to_string(),
                    directory: dir.to_string(),
                    parent_id: parent_id.to_string(),
                    created: now,
                    updated: now,
                    is_subagent: true,
                    ..Default::default()
                };
                sessions.insert(agent_id.to_string(), entry.clone());
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
        self.conns.close(id).await;
        let dir = self
            .sessions
            .lock()
            .ok()
            .and_then(|mut s| s.remove(id))
            .map(|e| e.directory);
        self.forget_derived(id);
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
            let Ok(mut sessions) = self.sessions.lock() else {
                return;
            };
            let Some(entry) = sessions.get_mut(id) else {
                return;
            };
            if (!manual && entry.title_locked) || entry.title == title {
                false
            } else {
                entry.title = title.to_string();
                entry.title_locked |= manual;
                entry.updated = now_ms();
                true
            }
        };
        if !changed {
            return;
        }
        self.save();
        if let Some(entry) = self.get_session(id) {
            self.emit(
                &entry.directory,
                "session.updated",
                json!({ "info": super::session_info(&entry) }),
            );
        }
    }

    /// Set busy and emit status; returns true on a busy→idle edge.
    pub fn set_busy(&self, id: &str, busy: bool) -> bool {
        let (dir, changed) = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return false;
            };
            let Some(entry) = sessions.get_mut(id) else {
                return false;
            };
            let changed = entry.busy != busy;
            entry.busy = busy;
            // Both edges of a turn are activity, so `updated` tracks the last
            // thing that happened in the session rather than only its creation.
            if changed {
                entry.updated = now_ms();
            }
            (entry.directory.clone(), changed)
        };
        if !changed {
            return false;
        }
        self.save();
        let status = if busy { "busy" } else { "idle" };
        self.emit(
            &dir,
            "session.status",
            json!({ "sessionID": id, "status": { "type": status } }),
        );
        if let Some(entry) = self.get_session(id) {
            self.emit(
                &dir,
                "session.updated",
                json!({ "info": super::session_info(&entry) }),
            );
        }
        if !busy {
            self.emit(&dir, "session.idle", json!({ "sessionID": id }));
        }
        !busy
    }

    // ── pending permission requests ──────────────────────────────────
    pub fn register_pending(&self, id: &str) -> tokio::sync::oneshot::Receiver<PendingReply> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(id.to_string(), tx);
        }
        rx
    }

    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let tx = self.pending.lock().ok().and_then(|mut p| p.remove(id));
        tx.map(|tx| tx.send(reply).is_ok()).unwrap_or(false)
    }

    // ── helpers ──────────────────────────────────────────────────────
    pub(super) fn with_discovered<T: Default>(&self, id: &str, f: impl FnOnce(&mut Discovered) -> T) -> T {
        let Ok(mut all) = self.discovered.lock() else {
            return T::default();
        };
        f(all.entry(id.to_string()).or_default())
    }

    pub(super) fn read_discovered<T: Default>(&self, id: &str, f: impl FnOnce(&Discovered) -> T) -> T {
        self.discovered
            .lock()
            .ok()
            .and_then(|all| all.get(id).map(f))
            .unwrap_or_default()
    }

    fn forget_derived(&self, id: &str) {
        if let Ok(mut all) = self.transcripts.lock() {
            all.remove(id);
        }
        if let Ok(mut all) = self.discovered.lock() {
            all.remove(id);
        }
        if let Ok(mut all) = self.followups.lock() {
            all.remove(id);
        }
        self.forget_hydrated(id);
    }

    pub(super) fn mutate(&self, id: &str, f: impl FnOnce(&mut Session)) -> Option<String> {
        let mut sessions = self.sessions.lock().ok()?;
        let entry = sessions.get_mut(id)?;
        f(entry);
        Some(entry.directory.clone())
    }

    pub(super) fn save(&self) {
        if let Ok(sessions) = self.sessions.lock() {
            super::session::save_sessions(&self.persist, &sessions);
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
