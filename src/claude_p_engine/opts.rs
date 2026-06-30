//! Engine URL, the PreToolUse hook settings, MCP attach config, the resolved
//! `claude -p` turn options, and the message-emit dedupe gate.

use serde_json::json;

use super::{default_model, ClaudePEngine};

impl ClaudePEngine {
    pub fn url(&self) -> String {
        self.url.lock().map(|u| u.clone()).unwrap_or_default()
    }

    pub(super) fn set_url(&self, url: &str) {
        if let Ok(mut u) = self.url.lock() {
            *u = url.to_string();
        }
    }

    fn hook_settings(&self) -> String {
        let cmd = format!("{} claude-hook", self.exe.to_string_lossy());
        json!({
            "hooks": { "PreToolUse": [ { "matcher": "*", "hooks": [ { "type": "command", "command": cmd } ] } ] }
        })
        .to_string()
    }

    fn mcp_config_json(&self, dir: &str, session_id: &str) -> Option<String> {
        let (terminal, neovim, time, ui) = self.mcp_flags;
        if !(terminal || neovim || time || ui) {
            return None;
        }
        let exe = self.exe.to_string_lossy().to_string();
        let env = json!({ "OPENCODE_SESSION_ID": session_id });
        let mut servers = serde_json::Map::new();
        if terminal {
            servers.insert("terminal".into(), json!({ "command": exe, "args": ["mcp", dir], "env": env }));
        }
        if neovim {
            servers.insert("neovim".into(), json!({ "command": exe, "args": ["mcp-nvim", dir], "env": env }));
        }
        if time {
            servers.insert("time".into(), json!({ "command": exe, "args": ["mcp-time"] }));
        }
        if ui {
            servers.insert("ui".into(), json!({ "command": exe, "args": ["mcp-ui"] }));
        }
        Some(json!({ "mcpServers": servers }).to_string())
    }

    /// Resolved options for a session's `claude -p` process.
    pub(super) fn turn_opts(&self, session_id: &str, dir: &str) -> super::process::TurnOpts {
        let s = self.get_session(session_id);
        super::process::TurnOpts {
            model: s.as_ref().and_then(|s| s.model.clone()).or_else(default_model),
            agent: s
                .as_ref()
                .and_then(|s| s.agent.clone())
                .map(|a| self.resolve_agent(session_id, &a))
                .filter(|a| !a.is_empty()),
            permission_mode: self.effective_mode(session_id),
            settings_json: self.hook_settings(),
            engine_url: self.url(),
            mcp_config: self.mcp_config_json(dir, session_id).unwrap_or_default(),
            session_env_id: session_id.to_string(),
            resume_uuid: self.resume_uuid(session_id),
        }
    }

    /// Content-hash gate for the stream reader: true if this message changed since the
    /// last emit (so unchanged messages aren't re-emitted on every line).
    pub fn should_emit(&self, session_id: &str, msg_id: &str, hash: u64) -> bool {
        let key = format!("{session_id}:{msg_id}");
        let mut g = match self.emitted.lock() {
            Ok(g) => g,
            Err(_) => return true,
        };
        if g.get(&key) == Some(&hash) {
            return false;
        }
        g.insert(key, hash);
        true
    }
}
