//! One renderer per runner wire shape. Each takes bound [`Wire`]s and emits the JSON
//! that runner expects — nothing else in the crate knows those shapes.
//!
//! The renderers differ in more than key names: OpenCode and Claude take a timeout in
//! milliseconds, while ACP has no timeout field at all (its schema carries none, so the
//! ceiling is set on the agent process's environment instead).

mod acp;
mod claude;
mod opencode;

pub use acp::servers as acp_servers;
pub use claude::config as claude_mcp_config;
pub use opencode::config as opencode_config;

use serde_json::{Map, Value};

use super::bind::WireStdio;

/// `{"NAME": "value"}` — the env shape Claude, Codex, and OpenCode all use.
fn env_object(wire: &WireStdio<'_>) -> Option<Value> {
    if wire.env.is_empty() {
        return None;
    }
    let mut map = Map::with_capacity(wire.env.len());
    for (name, value) in &wire.env {
        map.insert((*name).to_string(), Value::String(value.to_string()));
    }
    Some(Value::Object(map))
}

/// `{"NAME": "value"}` for headers, which every remote shape but ACP's uses.
fn header_object(headers: &[(&str, std::borrow::Cow<'_, str>)]) -> Value {
    let mut map = Map::with_capacity(headers.len());
    for (name, value) in headers {
        map.insert((*name).to_string(), Value::String(value.to_string()));
    }
    Value::Object(map)
}

fn args_array(wire: &WireStdio<'_>) -> Value {
    Value::Array(
        wire.args
            .iter()
            .map(|arg| Value::String(arg.to_string()))
            .collect(),
    )
}
