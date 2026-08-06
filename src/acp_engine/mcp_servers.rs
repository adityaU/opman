//! opman's own MCP servers, in ACP's `mcpServers` shape.
//!
//! Split from [`super`] so the engine module stays about the engine. Every ACP agent is
//! handed the same tool surface opman gives its other runners, which is why this is built
//! from the flags the host resolved at startup rather than from per-agent config — the one
//! per-agent say is `inject_mcp`, for an agent that cannot speak MCP at all.

use serde_json::{json, Value};

use super::AcpEngine;

impl AcpEngine {
    /// The server list for one session, or an empty list when injection is off.
    pub(super) fn mcp_servers(&self, dir: &str, session_id: &str) -> Value {
        if !self.agent.inject_mcp {
            return json!([]);
        }
        let (terminal, neovim, time, ui) = self.mcp_flags;
        let exe = self.exe.to_string_lossy().to_string();
        let env = |extra: Vec<(&str, String)>| -> Value {
            let mut vars = vec![json!({ "name": "OPENCODE_SESSION_ID", "value": session_id })];
            vars.extend(
                extra
                    .into_iter()
                    .map(|(name, value)| json!({ "name": name, "value": value })),
            );
            Value::Array(vars)
        };
        let mut servers = Vec::new();
        let mut stdio = |name: &str, args: Vec<String>, extra: Vec<(&str, String)>| {
            servers.push(json!({
                "name": name,
                "command": exe,
                "args": args,
                "env": env(extra),
            }));
        };
        if terminal {
            stdio("terminal", vec!["mcp".into(), dir.into()], vec![]);
        }
        if neovim {
            stdio("neovim", vec!["mcp-nvim".into(), dir.into()], vec![]);
        }
        if time {
            stdio("time", vec!["mcp-time".into()], vec![]);
        }
        if ui {
            stdio("ui", vec!["mcp-ui".into()], vec![]);
        }
        if let Ok(socket) = std::env::var("OPMAN_AGENT_MANAGER_SOCKET") {
            stdio(
                "agent-manager",
                vec!["mcp-agent-manager".into(), dir.into()],
                vec![("OPMAN_AGENT_MANAGER_SOCKET", socket)],
            );
        }
        Value::Array(servers)
    }
}
