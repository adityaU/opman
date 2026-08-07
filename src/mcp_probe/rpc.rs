//! One probe: spawn, handshake, list, kill.
//!
//! Deliberately not a general MCP client. It speaks the three messages a listing needs and
//! holds no state past the call, because the alternative — a pooled connection per declared
//! server — would keep every server the user has ever expanded running for the life of the
//! process.

use std::process::Stdio;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};

use super::{ServerInfo, ToolDef};
use crate::mcp_registry::bind::WireStdio;

/// The version opman's own built-ins answer with, so a server that negotiates gets the
/// same number from the settings page as it does from a runner.
const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug)]
pub(super) struct Listing {
    pub(super) server: Option<ServerInfo>,
    pub(super) tools: Vec<ToolDef>,
}

/// Launch `launch`, handshake, and return its `tools/list`.
///
/// The child is killed on the way out of this function whichever way it leaves: a server
/// that hangs mid-handshake must not outlive the request that started it.
pub(super) async fn list_tools(launch: &WireStdio<'_>) -> Result<Listing> {
    let mut child = spawn(launch)?;
    let mut stdin = child.stdin.take().context("server has no stdin")?;
    let stdout = child.stdout.take().context("server has no stdout")?;
    let mut lines = BufReader::new(stdout).lines();

    let hello = exchange(&mut stdin, &mut lines, 1, "initialize", initialize()).await?;
    // A notification takes no reply, so a failed write here is not itself the answer to
    // anything — the `tools/list` exchange below reports the dead server properly.
    let _ = write(
        &mut stdin,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )
    .await;
    let listed = exchange(&mut stdin, &mut lines, 2, "tools/list", json!({})).await?;

    // Best-effort: the child is `kill_on_drop`, so this only shortens the wait.
    let _ = child.start_kill();

    Ok(Listing {
        server: server_info(hello),
        tools: tools(listed)?,
    })
}

fn spawn(launch: &WireStdio<'_>) -> Result<tokio::process::Child> {
    let mut command = Command::new(launch.command);
    command
        .args(launch.args.iter().map(AsRef::as_ref))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // A server that logs to stderr would otherwise fill a pipe nobody drains and
        // block on its own diagnostics.
        .stderr(Stdio::null())
        .kill_on_drop(true);
    for (key, value) in &launch.env {
        command.env(key, value.as_ref());
    }
    if let Some(cwd) = &launch.cwd {
        command.current_dir(cwd.as_ref());
    }
    command
        .spawn()
        .with_context(|| format!("failed to launch `{}`", launch.command))
}

fn initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "opman-settings", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Send one request and read until its answer.
///
/// Anything arriving in between is a notification or a reply to a request this probe never
/// made; either way it is not the answer, so it is skipped rather than mistaken for one.
async fn exchange(
    stdin: &mut ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    id: u8,
    method: &str,
    params: Value,
) -> Result<Value> {
    // A write to a child's stdin fails for one reason: the child closed it, which on a
    // stdio server means it is gone. Reporting the resulting `EPIPE` verbatim would put
    // "Broken pipe (os error 32)" in front of a user whose actual problem is that their
    // server exits on startup.
    if write(
        stdin,
        &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
    )
    .await
    .is_err()
    {
        bail!("the server exited before answering `{method}`")
    }
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(mut message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message.get("id").and_then(Value::as_u64) != Some(u64::from(id)) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no detail");
            bail!("`{method}` was refused: {detail}");
        }
        return Ok(message
            .get_mut("result")
            .map(Value::take)
            .unwrap_or(Value::Null));
    }
    bail!("the server exited before answering `{method}`")
}

async fn write(stdin: &mut ChildStdin, message: &Value) -> Result<()> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await?;
    Ok(())
}

fn server_info(mut hello: Value) -> Option<ServerInfo> {
    let info = hello.get_mut("serverInfo").map(Value::take)?;
    serde_json::from_value(info).ok()
}

/// A malformed entry drops itself, not the listing: one tool with an unreadable schema is
/// no reason to tell the user the server answered nothing.
fn tools(mut listed: Value) -> Result<Vec<ToolDef>> {
    let Some(Value::Array(entries)) = listed.get_mut("tools").map(Value::take) else {
        bail!("`tools/list` answered without a `tools` array");
    };
    Ok(entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect())
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod rpc_tests;
