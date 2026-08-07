//! OpenCode's `OPENCODE_CONFIG_CONTENT`.
//!
//! What makes this the odd renderer out: it is built once for the whole `opencode serve`
//! process, so it binds with no session id and with the child's own working directory. A
//! server whose *positional* arguments need `${session}` cannot be offered here, and is
//! skipped rather than launched with a hole in its command line.
//!
//! OpenCode merges this payload with its own config files rather than being replaced by
//! it — verified by watching it still load `~/.config/opencode/opencode.json` with the
//! variable set — so only opman's own keys belong here.

use serde_json::{json, Map, Value};

use super::super::bind::{Bind, RemoteCaps, Wire};
use super::super::builtin::BuiltinFlags;
use super::super::spec::ServerSpec;
use super::{env_object, header_object};

pub fn config<'a>(
    servers: impl IntoIterator<Item = &'a ServerSpec>,
    at: Bind<'a>,
    flags: BuiltinFlags,
) -> anyhow::Result<String> {
    let mut root = Map::new();
    let mut mcp = Map::new();
    for spec in servers {
        match spec.bind(at, RemoteCaps::HTTP_ONLY) {
            Some(wire) => {
                mcp.insert(spec.name().to_string(), entry(spec, &wire));
            }
            None => tracing::debug!(
                server = spec.name(),
                "not offered to opencode; its config is process-wide and has no session"
            ),
        }
    }
    root.insert("mcp".into(), Value::Object(mcp));

    // Enabling opman's terminal or neovim bridge means denying OpenCode's own bash/edit,
    // or the model has two ways to do one thing. Derived from the flags, not the server
    // list, because a user-declared server should not change OpenCode's permissions.
    let mut permission = Map::new();
    if flags.terminal {
        permission.insert("bash".into(), json!("deny"));
    }
    if flags.neovim {
        permission.insert("edit".into(), json!("deny"));
    }
    if !permission.is_empty() {
        root.insert("permission".into(), Value::Object(permission));
    }
    Ok(serde_json::to_string(&Value::Object(root))?)
}

fn entry(spec: &ServerSpec, wire: &Wire<'_>) -> Value {
    let mut entry = Map::new();
    match wire {
        Wire::Stdio(stdio) => {
            entry.insert("type".into(), json!("local"));
            // OpenCode takes one flat command array rather than command plus args.
            let mut command = Vec::with_capacity(stdio.args.len() + 1);
            command.push(Value::String(stdio.command.to_string()));
            command.extend(stdio.args.iter().map(|arg| Value::String(arg.to_string())));
            entry.insert("command".into(), Value::Array(command));
            if let Some(env) = env_object(stdio) {
                entry.insert("environment".into(), env);
            }
            if let Some(cwd) = &stdio.cwd {
                entry.insert("cwd".into(), Value::String(cwd.to_string()));
            }
        }
        Wire::Remote(remote) => {
            entry.insert("type".into(), json!("remote"));
            entry.insert("url".into(), Value::String(remote.url.to_string()));
            entry.insert("headers".into(), header_object(&remote.headers));
        }
    }
    entry.insert("enabled".into(), json!(true));
    // Milliseconds, like Claude. OpenCode's default is 60s, and it *does* reset that on
    // progress notifications — but only a raised ceiling makes a long wait safe when a
    // server goes quiet.
    if let Some(secs) = spec.timeout_secs() {
        entry.insert("timeout".into(), json!(u64::from(secs) * 1000));
    }
    Value::Object(entry)
}

#[cfg(test)]
#[path = "opencode_tests.rs"]
mod opencode_tests;
