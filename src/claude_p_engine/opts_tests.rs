use super::*;
use crate::claude_engine::claude_cli::InitInfo;
use std::sync::Arc;

fn engine(flags: (bool, bool, bool, bool)) -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, flags))
}

#[test]
fn url_default_empty_then_set() {
    let e = engine((false, false, false, false));
    assert_eq!(e.url(), "");
    e.set_url("http://127.0.0.1:9");
    assert_eq!(e.url(), "http://127.0.0.1:9");
}

#[test]
fn hook_settings_shape() {
    let e = engine((false, false, false, false));
    let s = e.hook_settings();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "*");
    assert_eq!(v["worktree"]["bgIsolation"], "none");
    let cmd = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(cmd.ends_with("claude-hook"));
    assert_eq!(v["hooks"]["PreToolUse"][0]["hooks"][0]["type"], "command");
}

#[test]
fn mcp_config_none_when_all_flags_off() {
    let e = engine((false, false, false, false));
    assert!(e.mcp_config_json("dir", "sess").is_none());
}

#[test]
fn mcp_config_terminal_only() {
    let e = engine((true, false, false, false));
    let s = e.mcp_config_json("dir-x", "sess-1").unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let servers = &v["mcpServers"];
    assert!(servers.get("terminal").is_some());
    assert!(servers.get("neovim").is_none());
    assert_eq!(servers["terminal"]["args"][1], "dir-x");
    assert_eq!(servers["terminal"]["env"]["OPENCODE_SESSION_ID"], "sess-1");
}

#[test]
fn mcp_config_all_servers() {
    let e = engine((true, true, true, true));
    let s = e.mcp_config_json("d", "s").unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let servers = &v["mcpServers"];
    for k in ["terminal", "neovim", "time", "ui"] {
        assert!(servers.get(k).is_some(), "missing {k}");
    }
    // `time` and `ui` take no dir/env.
    assert_eq!(servers["time"]["args"][0], "mcp-time");
    assert_eq!(servers["ui"]["args"][0], "mcp-ui");
}

#[test]
fn mcp_config_neovim_and_time_only() {
    let e = engine((false, true, true, false));
    let s = e.mcp_config_json("d", "s").unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v["mcpServers"].get("neovim").is_some());
    assert!(v["mcpServers"].get("time").is_some());
    assert!(v["mcpServers"].get("terminal").is_none());
    assert!(v["mcpServers"].get("ui").is_none());
}

#[test]
fn turn_opts_uses_session_model_and_defaults() {
    let e = engine((false, false, false, false));
    let s = e.create_session("d1", "", "T");
    e.set_model(&s.id, "claude-custom");
    e.set_effort(&s.id, "high");
    let opts = e.turn_opts(&s.id, "d1");
    assert_eq!(opts.model, Some("claude-custom".to_string()));
    assert_eq!(opts.permission_mode, "bypassPermissions");
    assert_eq!(opts.effort.as_deref(), Some("high"));
    assert!(opts.settings_json.contains("PreToolUse"));
    assert_eq!(opts.engine_url, "");
    assert_eq!(opts.mcp_config, "");
    assert_eq!(opts.session_env_id, s.id);
    assert!(opts.resume_uuid.is_none());
    assert!(opts.agent.is_none());
}

#[test]
fn turn_opts_no_session_model_uses_default_model() {
    let e = engine((true, false, false, false));
    let s = e.create_session("d2", "", "T");
    e.set_url("http://x");
    let opts = e.turn_opts(&s.id, "d2");
    // No session model set → falls through the `.or_else(default_model)` branch.
    // (Value is env-driven; assert the other fields to stay deterministic.)
    assert_eq!(opts.engine_url, "http://x");
    assert!(!opts.mcp_config.is_empty(), "mcp flag on → config emitted");
    assert_eq!(opts.session_env_id, s.id);
}

#[test]
fn turn_opts_resolves_agent_from_cache() {
    let e = engine((false, false, false, false));
    let s = e.create_session("d3", "", "T");
    e.set_cached_init(
        "d3",
        InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
        },
    );
    e.set_agent(&s.id, "plan");
    let opts = e.turn_opts(&s.id, "d3");
    assert_eq!(opts.agent, Some("Plan".to_string()));
}

#[test]
fn should_emit_gate() {
    let e = engine((false, false, false, false));
    assert!(e.should_emit("s1", "m1", 100));
    assert!(!e.should_emit("s1", "m1", 100));
    assert!(e.should_emit("s1", "m1", 200));
    assert!(e.should_emit("s2", "m1", 100));
    assert!(e.should_emit("s1", "m2", 100));
}
