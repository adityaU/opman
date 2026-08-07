//! ACP's `mcpServers` array for `session/new` and `session/load`.
//!
//! Two shape quirks the other renderers do not have: env and headers are name/value
//! *pair objects* rather than maps, and stdio entries carry no `type` field — that is
//! what existing agents accept. There is also nowhere to put a timeout: ACP's schema has
//! no timeout on `McpServer`, `NewSessionRequest`, or anywhere else, and `configOptions`
//! ids are agent-defined rather than standardised. The ceiling for an ACP agent is set
//! on the agent process's own environment, via `acp.json`.

use serde_json::{json, Map, Value};

use super::super::bind::{Bind, RemoteCaps, Wire};
use super::super::spec::{RemoteKind, ServerSpec};

pub fn servers<'a>(
    servers: impl IntoIterator<Item = &'a ServerSpec>,
    at: Bind<'a>,
    caps: RemoteCaps,
) -> Value {
    let mut list = Vec::new();
    for spec in servers {
        let Some(wire) = spec.bind(at, caps) else {
            continue;
        };
        list.push(entry(spec.name(), &wire));
    }
    Value::Array(list)
}

fn entry(name: &str, wire: &Wire<'_>) -> Value {
    let mut entry = Map::new();
    entry.insert("name".into(), Value::String(name.to_string()));
    match wire {
        Wire::Stdio(stdio) => {
            entry.insert("command".into(), Value::String(stdio.command.to_string()));
            entry.insert(
                "args".into(),
                Value::Array(
                    stdio
                        .args
                        .iter()
                        .map(|arg| Value::String(arg.to_string()))
                        .collect(),
                ),
            );
            entry.insert("env".into(), pairs(stdio.env.iter().map(|(n, v)| (*n, v.as_ref()))));
        }
        Wire::Remote(remote) => {
            let kind = match remote.kind {
                RemoteKind::Http => "http",
                RemoteKind::Sse => "sse",
            };
            entry.insert("type".into(), Value::String(kind.into()));
            entry.insert("url".into(), Value::String(remote.url.to_string()));
            entry.insert(
                "headers".into(),
                pairs(remote.headers.iter().map(|(n, v)| (*n, v.as_ref()))),
            );
        }
    }
    Value::Object(entry)
}

/// ACP wants `[{"name": …, "value": …}]`, not an object.
fn pairs<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Value {
    Value::Array(
        entries
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect(),
    )
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod acp_tests;
