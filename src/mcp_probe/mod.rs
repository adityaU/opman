//! What a declared MCP server actually offers.
//!
//! `mcp.json` says how to reach a server; it says nothing about what that server exposes.
//! The only authority on that is the server itself, so this launches one exactly as a
//! runner would — through `opman mcp-proxy` whenever a credential or a remote transport is
//! involved — completes the handshake, and asks `tools/list`.
//!
//! Nothing is cached here. A probe is a one-shot child killed as soon as the listing is
//! read, so an edit to `mcp.json` shows up on the next request rather than on a restart.

mod rpc;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_registry::McpRegistry;

/// How long a server gets to finish the handshake and answer `tools/list`.
///
/// Generous for a local child, and deliberately far below the proxy's own ceiling: that
/// ceiling exists to hold a *tool call* open through a browser login, whereas a listing is
/// answered locally the moment a credential turns out to be missing.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Stands in for the session id a real launch would carry.
///
/// The session-bound servers — terminal, neovim, agent-manager — resolve `${session}` into
/// their argv, and dropping the server for want of an id would report "unavailable" for
/// three built-ins that list their tools perfectly well. None of them touch the id until a
/// tool is actually called, which a probe never does.
const PROBE_SESSION: &str = "probe";

/// One tool, verbatim as its server described it.
///
/// The schema fields pass through untouched: the page renders a parameter table from
/// `inputSchema` *and* offers the source, so anything dropped here would be a definition
/// the user asked for and did not get.
#[derive(Debug, Deserialize, Serialize)]
pub struct ToolDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        rename = "inputSchema",
        default,
        skip_serializing_if = "Value::is_null"
    )]
    pub input_schema: Value,
    #[serde(
        rename = "outputSchema",
        default,
        skip_serializing_if = "Value::is_null"
    )]
    pub output_schema: Value,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub annotations: Value,
}

/// The `serverInfo` from the handshake — the implementation's own name for itself, which
/// is not always the name it is declared under.
#[derive(Debug, Deserialize, Serialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The outcome of one probe.
///
/// A failure is part of the payload rather than an HTTP status: "this server is declared
/// but will not start" is exactly what the page needs to show, and a bare 502 cannot say
/// which of the three things went wrong.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Catalog {
    Listed {
        #[serde(skip_serializing_if = "Option::is_none")]
        server: Option<ServerInfo>,
        tools: Vec<ToolDef>,
    },
    /// opman declines to launch it at all: a presence condition is unmet, or a positional
    /// argument names something that does not exist.
    Unavailable { reason: &'static str },
    /// It was launched and did not answer.
    Failed { reason: String },
}

/// Ask one declared server for its tools.
pub async fn catalog(registry: &McpRegistry, name: &str, dir: &str) -> Catalog {
    let at = registry.bind(dir, Some(PROBE_SESSION));
    let Some(launch) = registry.stdio_launch(name, at) else {
        return Catalog::Unavailable {
            reason: "This server is not launchable right now — its presence condition is \
                     unmet, or an argument references something that does not exist.",
        };
    };
    match tokio::time::timeout(PROBE_TIMEOUT, rpc::list_tools(&launch)).await {
        Ok(Ok(listing)) => Catalog::Listed {
            server: listing.server,
            tools: listing.tools,
        },
        Ok(Err(error)) => Catalog::Failed {
            reason: format!("{error:#}"),
        },
        Err(_) => Catalog::Failed {
            reason: format!(
                "No reply within {}s. The server started but never answered the handshake.",
                PROBE_TIMEOUT.as_secs()
            ),
        },
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
