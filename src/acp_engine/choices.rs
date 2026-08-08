//! The engine choices a user makes for a session — model, mode, effort — and the agent
//! session id used to resume it.
//!
//! These are the persisted half of a session's configuration. What the *agent* offers for
//! each of them is discovered separately, in [`super::discovered`]; opman only pushes a
//! choice the agent has said it accepts.

use serde_json::{json, Value};

use super::{options, AcpEngine};

impl AcpEngine {
    // ── engine choices ───────────────────────────────────────────────
    pub fn set_model(&self, id: &str, model: &str) {
        if model.is_empty() {
            return;
        }
        self.mutate(id, |e| e.model = Some(model.to_string()));
        self.save();
    }

    pub fn set_agent(&self, id: &str, agent: &str) {
        let agent = agent.trim();
        self.mutate(id, |e| {
            e.agent = (!agent.is_empty()).then(|| agent.to_string())
        });
        self.save();
    }

    pub fn set_effort(&self, id: &str, effort: &str) {
        let effort = effort.trim();
        if effort.is_empty() {
            return;
        }
        self.mutate(id, |e| e.effort = Some(effort.to_string()));
        self.save();
    }

    pub fn set_permission_mode(&self, id: &str, mode: &str) {
        // The client sends a permission mode with every message, from a per-runner default it
        // keeps locally. For an agent whose modes are its agents there is nothing for that to
        // set, and storing it would fight `note_mode` over the same slot.
        if self.agent.modes_are_agents {
            return;
        }
        let dir = self.mutate(id, |e| e.permission_mode = Some(mode.to_string()));
        self.save();
        if let Some(dir) = dir {
            self.emit(
                &dir,
                "tui.toast.show",
                json!({ "message": format!("Permission mode: {mode}"), "variant": "info" }),
            );
        }
    }

    /// Record a mode the *agent* reports, without emitting a toast (the user did not ask).
    /// Which field it lands in depends on what this agent's modes are: writing `build` into
    /// a session's permission mode is the confusion `modes_are_agents` exists to avoid.
    pub fn note_mode(&self, id: &str, mode: &str) {
        let mode = mode.to_string();
        if self.agent.modes_are_agents {
            self.mutate(id, |e| e.agent = Some(mode));
        } else {
            self.mutate(id, |e| e.permission_mode = Some(mode));
        }
        self.save();
    }

    /// The value to push into the agent's `mode` config option, from whichever choice that
    /// slot represents for this agent.
    pub fn effective_mode(&self, id: &str) -> String {
        let session = self.get_session(id);
        let chosen = match self.agent.modes_are_agents {
            true => session.and_then(|s| s.agent),
            false => session.and_then(|s| s.permission_mode),
        };
        chosen.unwrap_or_else(|| self.agent.default_mode.clone())
    }

    /// The config options opman wants pushed onto a fresh session, as `(id, value)`.
    pub fn desired_options(&self, id: &str) -> Vec<(String, String)> {
        let Some(session) = self.get_session(id) else {
            return Vec::new();
        };
        [
            // Fall back to the agent's configured default so a session that has never had
            // a mode chosen still starts in the one config asked for.
            (options::MODE, Some(self.effective_mode(id))),
            (options::MODEL, session.model),
            (options::EFFORT, session.effort),
        ]
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value?;
            (!value.is_empty()).then(|| (key.to_string(), value))
        })
        .collect()
    }

    pub fn add_allowed_tool(&self, id: &str, tool: &str) {
        self.mutate(id, |e| {
            if !e.allowed_tools.iter().any(|t| t == tool) {
                e.allowed_tools.push(tool.to_string());
            }
        });
    }

    pub fn is_always_allowed(&self, id: &str, tool: &str) -> bool {
        self.get_session(id)
            .map(|s| s.allowed_tools.iter().any(|t| t == tool))
            .unwrap_or(false)
    }

    // ── agent session ids ────────────────────────────────────────────
    pub fn bind_acp_session(&self, id: &str, acp_session: &str) {
        if let Ok(mut map) = self.acp_ids.lock() {
            map.insert(acp_session.to_string(), id.to_string());
        }
        self.mutate(id, |e| e.acp_session = Some(acp_session.to_string()));
        self.save();
    }

    pub fn forget_acp_session(&self, id: &str) {
        let prior = self.get_session(id).and_then(|s| s.acp_session);
        if let (Ok(mut map), Some(prior)) = (self.acp_ids.lock(), prior) {
            map.remove(&prior);
        }
        self.mutate(id, |e| e.acp_session = None);
        self.save();
    }

    /// The opman session an inbound frame belongs to, resolved from its `sessionId`.
    pub fn opman_session(&self, params: &Value) -> Option<String> {
        let acp_session = params.get("sessionId").and_then(Value::as_str)?;
        self.acp_ids.lock().ok()?.get(acp_session).cloned()
    }
}
