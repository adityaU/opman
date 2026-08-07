//! Codex's `config.mcp_servers`, sent over JSON-RPC with `thread/start` and
//! `thread/resume`. Snake_case throughout, per Codex's convention.
//!
//! Codex is the runner that most needs an explicit timeout: measured, it stops at 300s
//! and — alone among the three — does not reset that clock on progress notifications, so
//! a held-open call dies there unless `tool_timeout_sec` is raised.

use serde_json::{json, Map, Value};

use super::super::bind::{Bind, RemoteCaps, Wire};
use super::super::spec::ServerSpec;
use super::{args_array, env_object, header_object};

pub fn config<'a>(servers: impl IntoIterator<Item = &'a ServerSpec>, at: Bind<'a>) -> Value {
    let mut map = Map::new();
    for spec in servers {
        // Codex speaks a single streamable-HTTP remote form, with no SSE flavour; an
        // SSE server therefore reaches it through the local proxy.
        let Some(wire) = spec.bind(at, RemoteCaps::HTTP_ONLY) else {
            continue;
        };
        map.insert(spec.name().to_string(), entry(spec, &wire));
    }
    json!({ "mcp_servers": Value::Object(map) })
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
            entry.insert("url".into(), Value::String(remote.url.to_string()));
            entry.insert("http_headers".into(), header_object(&remote.headers));
        }
    }
    if let Some(secs) = spec.timeout_secs() {
        entry.insert("tool_timeout_sec".into(), json!(secs));
    }
    Value::Object(entry)
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod codex_tests;
