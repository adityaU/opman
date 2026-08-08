//! `opman mcp-proxy <name>` — a stdio MCP server fronting one remote server.
//!
//! The runner speaks MCP to this process over stdio and never sees a credential; opman
//! owns the token lifecycle and attaches a fresh one per request. That is what makes
//! "authenticate once in opman" true across four runners, each of which otherwise has its
//! own OAuth silo.
//!
//! Transparent in both directions rather than mirroring a tool list: forwarding every
//! message means `resources/*`, `prompts/*`, and server-to-client requests all work with
//! no proxy code, and `initialize` is negotiated genuinely between the runner and the
//! remote.

mod upstream;
mod wait;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::mcp_oauth::{ServerName, TokenStore};
use upstream::{Upstream, UpstreamError};

pub async fn run_mcp_proxy(name: &str) -> anyhow::Result<()> {
    let server = ServerName::parse(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mode = Mode::resolve(&server).await;
    run_proxy_over(server, mode, tokio::io::stdin(), tokio::io::stdout()).await
}

/// `Authenticated` is the only state owning an [`Upstream`], so a missing credential
/// cannot become a half-attempted HTTP call.
pub(crate) enum Mode {
    Authenticated(Box<Upstream>),
    Degraded(DegradedReason),
}

#[derive(Clone, Debug)]
pub(crate) enum DegradedReason {
    /// No `mcp.json` entry, or it is not a remote server.
    NotConfigured,
    /// Configured, but opman holds no usable credential.
    NotAuthenticated,
}

impl DegradedReason {
    fn message(&self, name: &ServerName) -> String {
        match self {
            Self::NotConfigured => format!(
                "The MCP server \"{name}\" is not configured in opman. Add it in Settings, \
                 or in ~/.config/opman/mcp.json."
            ),
            Self::NotAuthenticated => format!(
                "opman is not authenticated to the MCP server \"{name}\". Ask the user to \
                 run `opman mcp login {name}`, or to click Log in on the {name} row in \
                 opman's Settings page. Then retry."
            ),
        }
    }
}

impl Mode {
    async fn resolve(name: &ServerName) -> Self {
        let registry =
            crate::mcp_registry::McpRegistry::load(crate::mcp_registry::BuiltinFlags::default());
        let Some(spec) = registry.get(name.as_str()) else {
            return Self::Degraded(DegradedReason::NotConfigured);
        };
        let crate::mcp_registry::spec::Transport::Remote(remote) = spec.transport() else {
            return Self::Degraded(DegradedReason::NotConfigured);
        };
        let Ok(store) = TokenStore::open() else {
            return Self::Degraded(DegradedReason::NotConfigured);
        };
        Self::Authenticated(Box::new(Upstream::new(name.clone(), remote, store)))
    }
}

pub(crate) async fn run_proxy_over<R, W>(
    name: ServerName,
    mode: Mode,
    reader: R,
    writer: W,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut writer = writer;
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                eprintln!("mcp-proxy: read error: {error}");
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            write_line(
                &mut writer,
                &json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": "Parse error" },
                    "id": Value::Null,
                }),
            )
            .await;
            continue;
        };
        // A notification has no id and takes no reply, so it must not produce one even
        // when we are degraded.
        let Some(id) = message.get("id").cloned().filter(|id| !id.is_null()) else {
            if let Mode::Authenticated(upstream) = &mode {
                upstream.notify(&message).await;
            }
            continue;
        };
        let response = match &mode {
            Mode::Authenticated(upstream) => handle(upstream, &name, &message, id).await,
            Mode::Degraded(reason) => vec![degraded_reply(&name, reason, &message, id)],
        };
        for value in response {
            write_line(&mut writer, &value).await;
        }
    }
    if let Mode::Authenticated(upstream) = &mode {
        upstream.terminate().await;
    }
    Ok(())
}

async fn handle(upstream: &Upstream, name: &ServerName, message: &Value, id: Value) -> Vec<Value> {
    match upstream.send(message).await {
        Ok(values) => values,
        // Only a tool call is worth holding open. The handshake and the listings must be
        // answered *now*, and answered locally — a runner whose `initialize` errors
        // treats the whole server as dead and drops it, after which nothing can tell the
        // user a login is all that was missing. Answering locally also means the server
        // recovers on its own once the credential lands, with no restart.
        Err(UpstreamError::NeedsLogin) if !is_tool_call(message) => {
            vec![degraded_reply(
                name,
                &DegradedReason::NotAuthenticated,
                message,
                id,
            )]
        }
        Err(UpstreamError::NeedsLogin) => {
            // Hold the call open rather than failing: emit progress while watching for
            // the credential to land, so the agent's call simply succeeds once the user
            // logs in. Falls back to an actionable error when it does not.
            wait::hold_open(upstream, name, message, id).await
        }
        Err(UpstreamError::NeedsScope(scope)) => vec![tool_error(
            &id,
            message,
            format!(
                "The MCP server \"{name}\" needs additional permissions ({scope}). Ask the \
                 user to run `opman mcp login {name}` to re-authorize."
            ),
        )],
        Err(UpstreamError::Transport(detail)) => vec![json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32003, "message": format!("MCP server \"{name}\" is unreachable: {detail}") },
        })],
    }
}

fn degraded_reply(name: &ServerName, reason: &DegradedReason, message: &Value, id: Value) -> Value {
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        // Answer the handshake locally: a runner that gets a dead stdio server drops the
        // whole entry, and then nothing can tell the user why.
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": format!("opman-proxy:{name}"), "version": env!("CARGO_PKG_VERSION") },
            }
        }),
        // One synthetic tool rather than an empty list, so the model can see the server
        // exists and report the problem.
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": [{
                "name": format!("{name}__authenticate"),
                "description": reason.message(name),
                "inputSchema": { "type": "object", "properties": {}, "required": [] },
            }]}
        }),
        "tools/call" => tool_error(&id, message, reason.message(name)),
        "resources/list" => json!({ "jsonrpc": "2.0", "id": id, "result": { "resources": [] } }),
        "prompts/list" => json!({ "jsonrpc": "2.0", "id": id, "result": { "prompts": [] } }),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32002, "message": reason.message(name) },
        }),
    }
}

fn is_tool_call(message: &Value) -> bool {
    message.get("method").and_then(Value::as_str) == Some("tools/call")
}

/// A *successful* result carrying `isError`, so the model reads the text and tells the
/// user, instead of the runner surfacing an opaque transport failure it cannot explain.
pub(crate) fn tool_error(id: &Value, message: &Value, text: String) -> Value {
    let is_call = message.get("method").and_then(Value::as_str) == Some("tools/call");
    if !is_call {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32002, "message": text },
        });
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }], "isError": true },
    })
}

async fn write_line<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) {
    let Ok(mut line) = serde_json::to_vec(value) else {
        return;
    };
    line.push(b'\n');
    let _ = writer.write_all(&line).await;
    let _ = writer.flush().await;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
