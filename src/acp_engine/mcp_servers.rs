//! opman's MCP servers, in ACP's `mcpServers` shape.
//!
//! Split from [`super`] so the engine module stays about the engine. Every ACP agent is
//! handed the same tool surface opman gives its other runners; the one per-agent say is
//! `inject_mcp`, for an agent that cannot speak MCP at all.
//!
//! ACP has no timeout anywhere in its schema — not on `McpServer`, not on
//! `NewSessionRequest` — and `configOptions` ids are agent-defined rather than
//! standardised. The tool-call ceiling for an ACP agent is therefore set on the agent
//! process's own environment, through `acp.json`.

use serde_json::Value;

use super::AcpEngine;
use crate::mcp_registry::{render, RemoteCaps};

/// What the agent said it can dial for itself, from its `initialize` reply.
///
/// Absent means unsupported — the spec's own default, and the safe reading for an agent
/// that predates the field. Anything it cannot dial reaches it as `opman mcp-proxy`
/// instead of being dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct McpCaps {
    http: bool,
    sse: bool,
}

impl McpCaps {
    pub(super) fn from_initialize(init: &Value) -> Self {
        let caps = init.pointer("/agentCapabilities/mcpCapabilities");
        let flag = |key: &str| {
            caps.and_then(|caps| caps.get(key))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            http: flag("http"),
            sse: flag("sse"),
        }
    }

    fn remote(self) -> RemoteCaps {
        RemoteCaps::new(self.http, self.sse)
    }
}

impl AcpEngine {
    /// The server list for one session, or an empty list when injection is off.
    pub(super) fn mcp_servers(&self, dir: &str, session_id: &str, caps: McpCaps) -> Value {
        if !self.agent.inject_mcp {
            return Value::Array(Vec::new());
        }
        let runner = crate::runner::RunnerKind::Acp(self.agent.runner.clone());
        let registry = self.mcp.current();
        render::acp_servers(
            registry.for_runner(&runner),
            registry.bind(dir, Some(session_id)),
            caps.remote(),
        )
    }
}
