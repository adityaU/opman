//! Holding a tool call open while the user authenticates.
//!
//! Failing fast and asking the agent to retry works, but it wastes a turn and often
//! confuses the model. Instead the proxy keeps the call open and emits
//! `notifications/progress`, which MCP explicitly permits a client to treat as a reason to
//! reset its timeout clock.
//!
//! Measured behaviour is why this is load-bearing rather than a nicety: OpenCode cancels a
//! silent call after 60 seconds and only survives longer *because* progress resets its
//! clock, while Codex ignores progress entirely and stops at 300 seconds unless opman
//! raises `tool_timeout_sec` — which the registry does for every proxied server.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::tool_error;
use super::upstream::{Upstream, UpstreamError};
use crate::mcp_oauth::ServerName;
use crate::mcp_registry::AUTH_WAIT_SECS;

/// How often to emit progress. Comfortably inside every measured base timeout.
const TICK: Duration = Duration::from_secs(10);

/// Wait for a credential to appear, then run the original call.
///
/// Emits progress only when the client supplied a `progressToken` — the spec permits
/// progress notifications solely for a token the client provided, so without one there is
/// nothing legal to send and the wait falls back to the actionable error.
pub(crate) async fn hold_open(
    upstream: &Upstream,
    name: &ServerName,
    message: &Value,
    id: Value,
) -> Vec<Value> {
    let token = message.pointer("/params/_meta/progressToken").cloned();
    if token.is_none() {
        return vec![tool_error(&id, message, needs_login(name))];
    }
    notify_parent(name);

    let deadline = Instant::now() + Duration::from_secs(AUTH_WAIT_SECS);
    let mut emitted = Vec::new();
    let mut step = 0_u64;
    while Instant::now() < deadline {
        if upstream.authenticated() {
            // Run the call for real. The agent never sees that anything went wrong.
            return match upstream.send(message).await {
                Ok(mut values) => {
                    emitted.append(&mut values);
                    emitted
                }
                Err(UpstreamError::NeedsLogin) => {
                    emitted.push(tool_error(&id, message, needs_login(name)));
                    emitted
                }
                Err(UpstreamError::NeedsScope(scope)) => {
                    emitted.push(tool_error(
                        &id,
                        message,
                        format!(
                            "The MCP server \"{name}\" needs additional permissions ({scope})."
                        ),
                    ));
                    emitted
                }
                Err(UpstreamError::Transport(detail)) => {
                    emitted.push(tool_error(
                        &id,
                        message,
                        format!("The MCP server \"{name}\" is unreachable: {detail}"),
                    ));
                    emitted
                }
            };
        }
        tokio::time::sleep(TICK).await;
        step += 1;
        emitted.push(progress(token.as_ref(), step, name));
    }
    emitted.push(tool_error(&id, message, timed_out(name)));
    emitted
}

/// `progress` must strictly increase, and `total` is omitted because the wait is
/// open-ended — the documented shape for work of unknown length.
fn progress(token: Option<&Value>, step: u64, name: &ServerName) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {
            "progressToken": token,
            "progress": step,
            "message": format!("waiting for you to log in to \"{name}\" in opman"),
        }
    })
}

fn needs_login(name: &ServerName) -> String {
    format!(
        "opman is not authenticated to the MCP server \"{name}\". Ask the user to run \
         `opman mcp login {name}`, or to click Log in on the {name} row in opman's Settings \
         page. Then retry this tool."
    )
}

fn timed_out(name: &ServerName) -> String {
    format!(
        "Timed out waiting for the user to authenticate to \"{name}\". Ask them to run \
         `opman mcp login {name}` and then retry this tool."
    )
}

/// Best-effort nudge so opman can surface a login prompt while the call waits.
///
/// Fire and forget by design: the socket is PID-scoped and reaches this child only by
/// environment inheritance, so a proxy the user wired into a runner's own config by hand
/// simply will not have it. The tool text above stays the load-bearing path.
fn notify_parent(name: &ServerName) {
    let Ok(socket) = std::env::var(crate::mcp_agent_manager::SOCKET_ENV) else {
        return;
    };
    let payload = json!({ "op": "mcp_auth_required", "server": name.as_str() }).to_string();
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        if let Ok(mut stream) = tokio::net::UnixStream::connect(&socket).await {
            let _ = stream.write_all(payload.as_bytes()).await;
            let _ = stream.write_all(b"\n").await;
            let _ = stream.flush().await;
        }
    });
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod wait_tests;
