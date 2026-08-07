//! Claude Code's `--mcp-config` payload.
//!
//! Passed inline on argv rather than written to a file, so opman never touches
//! `~/.claude.json`. Note that Claude *merges* this with the user's own configuration
//! unless `--strict-mcp-config` is given — which opman deliberately does not pass, since
//! that would silently drop every server the user configured for themselves.

use serde_json::{json, Map, Value};

use super::super::bind::{Bind, RemoteCaps, Wire};
use super::super::spec::{RemoteKind, ServerSpec};
use super::{args_array, env_object, header_object};

/// `{"mcpServers": {…}}`, or `None` when nothing is offered so the flag can be omitted
/// entirely rather than passed an empty object.
pub fn config<'a>(
    servers: impl IntoIterator<Item = &'a ServerSpec>,
    at: Bind<'a>,
) -> Option<String> {
    let mut map = Map::new();
    for spec in servers {
        let Some(wire) = spec.bind(at, RemoteCaps::CLAUDE) else {
            continue;
        };
        map.insert(spec.name().to_string(), entry(spec, &wire));
    }
    if map.is_empty() {
        return None;
    }
    serde_json::to_string(&json!({ "mcpServers": Value::Object(map) })).ok()
}

fn entry(spec: &ServerSpec, wire: &Wire<'_>) -> Value {
    let mut entry = Map::new();
    match wire {
        Wire::Stdio(stdio) => {
            entry.insert("command".into(), Value::String(stdio.command.to_string()));
            entry.insert("args".into(), args_array(stdio));
            if let Some(env) = env_object(stdio) {
                entry.insert("env".into(), env);
            }
            if let Some(cwd) = &stdio.cwd {
                entry.insert("cwd".into(), Value::String(cwd.to_string()));
            }
        }
        Wire::Remote(remote) => {
            let kind = match remote.kind {
                RemoteKind::Http => "http",
                RemoteKind::Sse => "sse",
            };
            entry.insert("type".into(), Value::String(kind.into()));
            entry.insert("url".into(), Value::String(remote.url.to_string()));
            entry.insert("headers".into(), header_object(&remote.headers));
        }
    }
    // Claude's per-server timeout is in milliseconds.
    if let Some(secs) = spec.timeout_secs() {
        entry.insert("timeout".into(), json!(u64::from(secs) * 1000));
    }
    Value::Object(entry)
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod claude_tests;
