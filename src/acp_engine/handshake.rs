//! `initialize`, and the login it is allowed to demand.
//!
//! Split from [`super::conn`], which owns what happens *after* the two sides agree: this is
//! only the agreement. Everything opman later decides per connection — whether history can be
//! replayed, whether a prompt may interrupt a turn, which content blocks may be sent, which
//! MCP transports the agent dials itself — is read out of the one `initialize` reply, so
//! reading them in one place keeps the connection code about connecting.

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::attach::PromptCaps;
use super::jsonrpc::Peer;
use super::mcp_servers::McpCaps;
use super::AcpEngine;

/// The ACP revision opman speaks.
pub const PROTOCOL_VERSION: u64 = 1;

/// What one handshake settled.
pub(super) struct Negotiated {
    /// The reply itself, kept for the auth methods a later `session/new` may need.
    pub init: Value,
    /// Whether a prompt may be sent while a turn is running.
    pub steering: bool,
    /// Which non-text content blocks the agent will accept in a prompt.
    pub prompt_caps: PromptCaps,
    /// Which remote MCP transports the agent can dial for itself.
    pub mcp_caps: McpCaps,
    /// Whether an old conversation can be replayed with `session/load`.
    pub loads: bool,
}

/// Introduce opman and read back what the agent can do.
pub(super) async fn negotiate(engine: &Arc<AcpEngine>, peer: &Peer) -> Result<Negotiated> {
    let init = peer
        .request("initialize", params(engine))
        .await
        .context("ACP `initialize` failed")?;
    agreed_version(&init)?;
    Ok(Negotiated {
        steering: advertises_steering(&init),
        prompt_caps: PromptCaps::from_initialize(&init),
        mcp_caps: McpCaps::from_initialize(&init),
        loads: supports_load(&init),
        init,
    })
}

fn params(engine: &Arc<AcpEngine>) -> Value {
    let caps = &engine.agent.client_caps;
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "clientCapabilities": {
            "fs": {
                "readTextFile": caps.read_text_file,
                "writeTextFile": caps.write_text_file,
            },
            "terminal": caps.terminal,
        },
        "clientInfo": { "name": "opman", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Check the version the agent settled on.
///
/// ACP has the agent answer with the newest revision it speaks that is no newer than the
/// client's, so anything *higher* than opman asked for is an agent that will send frames
/// opman does not understand — better said once here than discovered one missing field at a
/// time. A lower answer is left alone: it means an agent older than opman, which the rest of
/// this module already handles by treating every absent field as unsupported.
fn agreed_version(init: &Value) -> Result<()> {
    // Absent means an agent from before the field existed, which can only be v1.
    let version = init
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .unwrap_or(PROTOCOL_VERSION);
    if version > PROTOCOL_VERSION {
        bail!("agent speaks ACP v{version}; opman speaks v{PROTOCOL_VERSION}");
    }
    Ok(())
}

fn supports_load(init: &Value) -> bool {
    init.get("agentCapabilities")
        .and_then(|c| c.get("loadSession"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether a prompt may be sent while a turn is running. Both spellings seen in the wild:
/// a top-level `_meta.steering` marker and a per-agent `promptQueueing` flag.
fn advertises_steering(init: &Value) -> bool {
    let steering = init
        .get("_meta")
        .and_then(|m| m.get("steering"))
        .and_then(|s| s.get("supported"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let queueing = init
        .get("agentCapabilities")
        .and_then(|c| c.get("_meta"))
        .and_then(|m| m.get("claudeCode"))
        .and_then(|c| c.get("promptQueueing"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    steering || queueing
}

/// Log in, after the agent has refused a session for want of a credential.
///
/// ACP leaves the choice of method to the client and gives it nothing to choose on but an id
/// and a label. opman takes the first the agent lists — agents publish one in practice — and
/// returns which, so a wrong guess shows up in the error rather than as a silent failure to
/// start. An agent that demands authentication without saying how cannot be helped from here:
/// its own CLI owns that flow.
pub(super) async fn authenticate(peer: &Peer, init: &Value) -> Result<String> {
    let Some(method) = first_auth_method(init) else {
        bail!("the agent requires authentication but advertised no `authMethods` — log in with the agent's own CLI first");
    };
    peer.request("authenticate", json!({ "methodId": method }))
        .await
        .with_context(|| format!("ACP `authenticate` failed for method `{method}`"))?;
    Ok(method.to_string())
}

/// The id opman will log in with. Empty ids are skipped rather than sent: `authenticate` is
/// keyed on the id, and an empty one names nothing.
fn first_auth_method(init: &Value) -> Option<&str> {
    init.get("authMethods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
        })
}

#[cfg(test)]
#[path = "handshake_tests.rs"]
mod handshake_tests;
