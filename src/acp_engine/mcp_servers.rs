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
        let caps = [
            "/agentCapabilities/mcpCapabilities",
            "/agentCapabilities/mcp",
            "/mcpCapabilities",
            "/capabilities/mcpCapabilities",
        ]
        .into_iter()
        .find_map(|path| init.pointer(path));
        let flag = |key: &str| {
            caps.and_then(|caps| caps.get(key)).is_some_and(|value| match value {
                Value::Bool(value) => *value,
                // A few ACP bridges publish capability objects instead of booleans.
                Value::Object(_) => true,
                _ => false,
            })
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

#[cfg(test)]
mod tests {
    use super::McpCaps;
    use serde_json::json;

    #[test]
    fn accepts_standard_and_bridge_capability_locations() {
        for init in [
            json!({"agentCapabilities":{"mcpCapabilities":{"http":true,"sse":true}}}),
            json!({"agentCapabilities":{"mcp":{"http":true,"sse":true}}}),
            json!({"mcpCapabilities":{"http":true,"sse":true}}),
            json!({"capabilities":{"mcpCapabilities":{"http":true,"sse":true}}}),
        ] {
            assert_eq!(McpCaps::from_initialize(&init), McpCaps { http: true, sse: true });
        }
    }

    #[test]
    fn object_capabilities_are_supported() {
        let init = json!({"agentCapabilities":{"mcpCapabilities":{"http":{},"sse":false}}});
        assert_eq!(McpCaps::from_initialize(&init), McpCaps { http: true, sse: false });
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
