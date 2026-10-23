//! ACP's `mcpServers` array for `session/new` and `session/load`.
//!
//! Two shape quirks the other renderers do not have: env and headers are name/value
//! *pair objects* rather than maps, and stdio entries carry no `type` field — that is
//! what existing agents accept. There is also nowhere to put a timeout: ACP's schema has
//! no timeout on `McpServer`, `NewSessionRequest`, or anywhere else, and `configOptions`
//! ids are agent-defined rather than standardised. The ceiling for an ACP agent is set
//! on the agent process's own environment, via `acp.json`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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
        list.push(entry(spec.name(), &wire, Path::new(at.dir())));
    }
    Value::Array(list)
}

fn entry(name: &str, wire: &Wire<'_>, session_dir: &Path) -> Value {
    let mut entry = Map::new();
    entry.insert("name".into(), Value::String(name.to_string()));
    match wire {
        Wire::Stdio(stdio) => {
            entry.insert(
                "command".into(),
                Value::String(absolute_command(
                    stdio.command,
                    std::env::var_os("PATH").as_deref(),
                    session_dir,
                )),
            );
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
            entry.insert(
                "env".into(),
                pairs(stdio.env.iter().map(|(n, v)| (*n, v.as_ref()))),
            );
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

/// ACP requires stdio MCP commands to be absolute paths. Resolve PATH commands and
/// session-relative executable paths for agents that enforce that requirement strictly.
fn absolute_command(command: &str, search_path: Option<&OsStr>, session_dir: &Path) -> String {
    let path = Path::new(command);
    if path.is_absolute() {
        return command.to_string();
    }
    let absolute_dir = absolute_session_dir(session_dir);
    if path.components().count() > 1 {
        return absolute_dir.join(path).to_string_lossy().into_owned();
    }

    if let Some(search_path) = search_path {
        for directory in std::env::split_paths(search_path) {
            let directory = if directory.is_absolute() {
                directory
            } else {
                absolute_dir.join(directory)
            };
            let candidate = directory.join(path);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    // ACP v1 rejects a relative command before it starts the MCP. Keep the
    // entry schema-valid when PATH cannot resolve it; the process still gets
    // the useful absolute location in its eventual launch error.
    absolute_dir.join(path).to_string_lossy().into_owned()
}

fn absolute_session_dir(session_dir: &Path) -> PathBuf {
    if session_dir.is_absolute() {
        return session_dir.to_path_buf();
    }
    if let Ok(current) = std::env::current_dir() {
        return current.join(session_dir);
    }
    PathBuf::from("/").join(session_dir)
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

#[cfg(test)]
mod command_tests {
    use super::absolute_command;
    use std::path::Path;

    #[test]
    fn resolves_a_path_command_against_the_session_directory() {
        let command = absolute_command("./tools/mcp", None, Path::new("/workspace/project"));
        assert_eq!(command, "/workspace/project/./tools/mcp");
    }

    #[test]
    fn resolves_a_bare_command_from_path() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("mcp-server");
        std::fs::write(&executable, "")?;
        let command = absolute_command(
            "mcp-server",
            Some(directory.path().as_os_str()),
            Path::new("/workspace/project"),
        );
        assert_eq!(command, executable.to_string_lossy());
        Ok(())
    }

    #[test]
    fn preserves_an_absolute_command() {
        let command = absolute_command("/opt/mcp-server", None, Path::new("/workspace/project"));
        assert_eq!(command, "/opt/mcp-server");
    }

    #[test]
    fn keeps_an_unresolved_bare_command_absolute() {
        let command =
            absolute_command("missing-mcp-command", None, Path::new("/workspace/project"));
        assert!(Path::new(&command).is_absolute());
    }
}
