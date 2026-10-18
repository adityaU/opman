//! Turning a [`NativeTools`](super::native_tools::NativeTools) choice into what the agent
//! is actually given: session metadata for Claude, environment for opencode and Codex.
//!
//! Both are computed at launch and session start, so a change reaches new sessions only.

use serde_json::{json, Map, Value};

use super::config::AgentConfig;
use super::native_tools::Mechanism;

const OPENCODE_ENV: &str = "OPENCODE_CONFIG_CONTENT";
const CODEX_ENV: &str = "CODEX_CONFIG";

impl AgentConfig {
    /// The `_meta` to send with `session/new` and `session/load`, when this agent takes
    /// its disabled tools there.
    pub fn session_meta(&self) -> Option<Value> {
        let (set, ids) = self.disabled_ids()?;
        if set.mechanism != Mechanism::ClaudeSessionMeta {
            return None;
        }
        Some(json!({ "claudeCode": { "options": { "disallowedTools": ids } } }))
    }

    /// An environment variable to set on the child, when this agent takes its disabled
    /// tools there. A value the user already put in `env` is kept and the switches are
    /// merged over it, so an opencode that also has permission rules keeps them.
    pub fn tool_env(&self) -> Option<(&'static str, String)> {
        let (set, ids) = self.disabled_ids()?;
        let (key, overlay) = match set.mechanism {
            Mechanism::ClaudeSessionMeta => return None,
            Mechanism::OpencodePermission => (OPENCODE_ENV, opencode_overlay(&ids)),
            Mechanism::CodexConfig => (CODEX_ENV, codex_overlay(&ids)),
        };
        let mut base = self.existing_json(key);
        if set.mechanism == Mechanism::OpencodePermission {
            expand_blanket_permission(&mut base);
        }
        merge(&mut base, overlay);
        Some((key, base.to_string()))
    }

    /// The disabled tool ids, or `None` when nothing is switched off or the harness has no
    /// way to switch anything off.
    fn disabled_ids(&self) -> Option<(&'static super::native_tools::ToolSet, Vec<&'static str>)> {
        let set = self.tool_set?;
        let ids: Vec<&'static str> = self
            .native_tools
            .disabled(set)
            .into_iter()
            .map(|tool| tool.id)
            .collect();
        (!ids.is_empty()).then_some((set, ids))
    }

    /// The JSON object the user already set in `env[key]`, or an empty one. A value that
    /// is not a JSON object could not be merged into, so it is dropped with a warning.
    fn existing_json(&self, key: &str) -> Value {
        let Some(raw) = self.env.get(key) else {
            return Value::Object(Map::new());
        };
        match serde_json::from_str::<Value>(raw) {
            Ok(value) if value.is_object() => value,
            _ => {
                tracing::warn!("ignoring {key} in the agent env: it is not a JSON object");
                Value::Object(Map::new())
            }
        }
    }
}

fn opencode_overlay(ids: &[&str]) -> Value {
    let rules: Map<String, Value> = ids
        .iter()
        .map(|id| ((*id).to_string(), json!("deny")))
        .collect();
    json!({ "permission": rules })
}

/// Codex ids are config paths (`features.shell_tool`); `web_search` is a mode, not a flag.
fn codex_overlay(ids: &[&str]) -> Value {
    let mut overlay = Value::Object(Map::new());
    for id in ids {
        let off = if *id == "web_search" {
            json!("disabled")
        } else {
            json!(false)
        };
        merge(&mut overlay, nest(id, off));
    }
    overlay
}

/// `a.b` and `1` become `{"a":{"b":1}}`.
fn nest(path: &str, leaf: Value) -> Value {
    path.rsplit('.').fold(leaf, |inner, key| {
        let mut object = Map::new();
        object.insert(key.to_string(), inner);
        Value::Object(object)
    })
}

/// opencode also accepts `"permission": "allow"` for everything. Merging a rule into a
/// string would lose it, so it becomes the equivalent `{"*": "allow"}` first.
fn expand_blanket_permission(config: &mut Value) {
    let Some(permission) = config.get_mut("permission") else {
        return;
    };
    if let Value::String(action) = permission {
        *permission = json!({ "*": action });
    }
}

/// Deep merge: objects merge key by key, anything else in `overlay` wins.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                merge(base.entry(key).or_insert(Value::Null), value);
            }
        }
        (slot, value) => *slot = value,
    }
}

#[cfg(test)]
#[path = "native_tools_render_tests.rs"]
mod native_tools_render_tests;
