//! What the agent told opman about itself.
//!
//! Models, permission modes, slash commands and the task list are all reported over ACP
//! rather than configured, so they are rebuilt on every connection instead of persisted.
//! Keeping them here is what stops agent capability leaking into the session registry.

use serde_json::Value;

use super::{options, AcpEngine};

impl AcpEngine {
    /// Store the startup probe's `session/new` reply.
    pub fn set_capabilities(&self, setup: Value) {
        if let Ok(mut cached) = self.capabilities.lock() {
            *cached = setup;
        }
    }

    /// Capability lookup across every source, newest first: a live session's own reply, then
    /// the startup probe. A live session wins because the user may have changed the mode or
    /// model since startup.
    fn any_session<T, F>(&self, read: F) -> T
    where
        T: Default + IsEmpty,
        F: Fn(&Value) -> T,
    {
        let ids: Vec<String> = self
            .sessions
            .lock()
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default();
        let found = ids
            .iter()
            .map(|id| self.read_discovered(id, |d| read(&d.setup)))
            .find(|value| !value.is_empty());
        if let Some(value) = found {
            return value;
        }
        self.capabilities
            .lock()
            .map(|setup| read(&setup))
            .unwrap_or_default()
    }
    // ── agent-reported state ─────────────────────────────────────────
    /// Store the `session/new` / `session/load` reply and adopt the agent's own mode — only
    /// where the session has not been given one, since opman's choice is about to be pushed.
    pub fn merge_session_setup(&self, id: &str, setup: &Value) {
        self.with_discovered(id, |d| d.setup = setup.clone());
        if let Some(mode) = options::current_mode(setup) {
            self.adopt_mode(id, &mode);
        }
    }

    /// Fold a `config_option_update` into the stored setup so `/provider` and the picker
    /// reflect what the agent currently offers.
    pub fn merge_config_options(&self, id: &str, update: &Value) {
        let Some(option_id) = update.get("configId").and_then(Value::as_str) else {
            return;
        };
        let value = update.get("value").cloned();
        self.with_discovered(id, |d| {
            let Some(list) = d
                .setup
                .get_mut("configOptions")
                .and_then(Value::as_array_mut)
            else {
                return;
            };
            let found: Option<&mut Value> = list
                .iter_mut()
                .find(|o| o.get("id").and_then(Value::as_str) == Some(option_id));
            if let (Some(entry), Some(value)) = (found, value) {
                entry["currentValue"] = value;
            }
        });
        if option_id == options::MODE {
            if let Some(mode) = update.get("value").and_then(Value::as_str) {
                self.note_mode(id, mode);
            }
        }
    }

    /// Record a choice the agent just accepted, from whichever channel set it.
    ///
    /// The three set methods answer differently: `session/set_config_option` returns the whole
    /// reconciled list, while the spec's two return nothing at all. Folding both back here is
    /// what keeps a later sync from re-pushing a value the agent is already on.
    pub fn note_selected(
        &self,
        id: &str,
        channel: options::Channel,
        option: &str,
        value: &str,
        reply: &Value,
    ) {
        match channel {
            options::Channel::Config => self.merge_config_list(id, reply),
            spec => self.with_discovered(id, |d| options::note_current(&mut d.setup, spec, value)),
        }
        if option == options::MODE {
            self.note_mode(id, value);
        }
    }

    /// Replace the stored `configOptions` with a fresher list — the reply to
    /// `session/set_config_option` carries the whole array, already reconciled by the agent.
    pub fn merge_config_list(&self, id: &str, reply: &Value) {
        let Some(list) = reply.get("configOptions") else {
            return;
        };
        self.with_discovered(id, |d| d.setup["configOptions"] = list.clone());
    }

    /// This session's `session/new` reply, for comparing against what the agent has now.
    /// Unlike [`Self::modes`] this is deliberately not cross-session: config options are
    /// per-session state, and reading another session's would push spurious changes.
    pub fn session_setup(&self, id: &str) -> Value {
        self.read_discovered(id, |d| d.setup.clone())
    }

    pub fn set_commands(&self, id: &str, commands: Vec<Value>) {
        self.with_discovered(id, |d| d.commands = commands);
    }

    pub fn set_todos(&self, id: &str, todos: Vec<Value>) {
        self.with_discovered(id, |d| d.todos = todos);
    }

    pub fn todos(&self, id: &str) -> Vec<Value> {
        self.read_discovered(id, |d| d.todos.clone())
    }

    /// Slash commands for a directory: any live session in it has the same set, since they
    /// come from the same project.
    pub fn commands_for_dir(&self, dir: &str) -> Vec<Value> {
        self.list_for_dir(dir)
            .iter()
            .map(|s| self.read_discovered(&s.id, |d| d.commands.clone()))
            .find(|commands| !commands.is_empty())
            .unwrap_or_default()
    }

    /// The models the agent offers.
    pub fn models(&self) -> Vec<options::Choice> {
        self.any_session(options::models)
    }

    /// The reasoning effort choices the agent exposes through `configOptions`.
    pub fn efforts(&self) -> Vec<options::Choice> {
        self.any_session(options::efforts)
    }

    /// The model the agent currently has selected, for the picker's default.
    pub fn current_model(&self) -> Option<String> {
        self.any_session(options::current_model)
    }

    /// The permission modes the agent offers, for the engine picker.
    pub fn modes(&self) -> Vec<options::Choice> {
        self.any_session(options::mode_ids)
    }
}

/// "Nothing discovered yet", so the lookup can keep looking. A trait rather than an
/// `is_empty` closure per call site: `Option` and `Vec` mean it differently.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

impl<T> IsEmpty for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}
