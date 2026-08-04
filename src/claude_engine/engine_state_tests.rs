//! Generated coverage tests for `mod.rs` — pure fns + engine state methods.
use super::registry::SessionEntry;
use super::*;

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

// ── free functions ──────────────────────────────────────────────────

#[test]
fn now_ms_and_rand_id() {
    assert!(now_ms() > 0);
    let a = rand_id("ses");
    assert!(a.starts_with("ses_"));
    assert_eq!(a.len(), "ses_".len() + 32);
    assert_ne!(rand_id("x"), rand_id("x"));
}

#[test]
fn default_model_and_kanban_flag_are_callable() {
    // Exercise the bodies (values depend on env / real config, so just call them).
    let _ = default_model();
    let _ = kanban_internal_available();
}

#[test]
fn session_info_shape() {
    let entry = SessionEntry {
        id: "ses_9".into(),
        title: "T".into(),
        directory: "/d".into(),
        parent_id: "p".into(),
        created: 10,
        updated: 20,
        ..Default::default()
    };
    let v = session_info(&entry);
    assert_eq!(v["id"], "ses_9");
    assert_eq!(v["title"], "T");
    assert_eq!(v["parentID"], "p");
    assert_eq!(v["directory"], "/d");
    assert_eq!(v["time"]["created"], 10);
    assert_eq!(v["time"]["updated"], 20);
}

// ── mcp_config_json ─────────────────────────────────────────────────

#[test]
fn mcp_config_none_when_all_disabled() {
    let e = engine();
    // Only assert None when the kanban internal descriptor is absent (test env).
    if !kanban_internal_available() {
        assert!(e.mcp_config_json("/d", "ses1").is_none());
    }
}

#[test]
fn mcp_config_builds_each_server() {
    let terminal = Arc::new(ClaudeEngine::new(None, (true, false, false, false)));
    let cfg: serde_json::Value =
        serde_json::from_str(&terminal.mcp_config_json("/dir", "ses1").unwrap()).unwrap();
    let t = &cfg["mcpServers"]["terminal"];
    assert!(t["command"].is_string());
    assert_eq!(t["args"][0], "mcp");
    assert_eq!(t["args"][1], "/dir");
    assert_eq!(t["env"]["OPENCODE_SESSION_ID"], "ses1");

    let neovim = Arc::new(ClaudeEngine::new(None, (false, true, false, false)));
    let cfg: serde_json::Value =
        serde_json::from_str(&neovim.mcp_config_json("/dir", "s").unwrap()).unwrap();
    assert_eq!(cfg["mcpServers"]["neovim"]["args"][0], "mcp-nvim");

    let time = Arc::new(ClaudeEngine::new(None, (false, false, true, false)));
    let cfg: serde_json::Value =
        serde_json::from_str(&time.mcp_config_json("/dir", "s").unwrap()).unwrap();
    assert_eq!(cfg["mcpServers"]["time"]["args"][0], "mcp-time");

    let ui = Arc::new(ClaudeEngine::new(None, (false, false, false, true)));
    let cfg: serde_json::Value =
        serde_json::from_str(&ui.mcp_config_json("/dir", "s").unwrap()).unwrap();
    assert_eq!(cfg["mcpServers"]["ui"]["args"][0], "mcp-ui");
}

// ── url / exe / mode / model ────────────────────────────────────────

#[test]
fn url_and_exe_accessors() {
    let e = engine();
    assert_eq!(e.url(), "");
    e.set_url("http://127.0.0.1:9999");
    assert_eq!(e.url(), "http://127.0.0.1:9999");
    assert!(!e.exe().as_os_str().is_empty());
}

#[test]
fn effective_mode_default_and_override() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert_eq!(e.effective_mode(&s.id), "bypassPermissions");
    e.set_permission_mode(&s.id, "plan");
    assert_eq!(e.effective_mode(&s.id), "plan");
    // Missing session → engine default.
    assert_eq!(e.effective_mode("nope"), "bypassPermissions");
}

#[tokio::test]
async fn set_permission_mode_emits_toast() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    let mut rx = e.subscribe();
    e.set_permission_mode(&s.id, "acceptEdits");
    let ev = rx.recv().await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "tui.toast.show");
    // A missing session is a no-op (no panic).
    e.set_permission_mode("nope", "plan");
}

#[test]
fn set_model_semantics() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    e.set_model(&s.id, ""); // empty → ignored
    assert_eq!(e.get_session(&s.id).unwrap().model, None);
    e.set_model(&s.id, "opus");
    assert_eq!(e.get_session(&s.id).unwrap().model.as_deref(), Some("opus"));
    e.set_model(&s.id, "opus"); // unchanged → no-op branch
    assert_eq!(e.get_session(&s.id).unwrap().model.as_deref(), Some("opus"));
    e.set_model("missing", "x"); // no session → no-op
}

// ── resolve_agent ───────────────────────────────────────────────────

#[test]
fn resolve_agent_all_branches() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    assert_eq!(e.resolve_agent(&s.id, ""), ""); // empty
                                                // No cached init yet (known empty): plan → "Plan", unknown → passthrough.
    assert_eq!(e.resolve_agent(&s.id, "plan"), "Plan");
    assert_eq!(e.resolve_agent(&s.id, "myproj-agent"), "myproj-agent");
    assert_eq!(e.resolve_agent(&s.id, "build"), ""); // alias → default

    // With a real known list.
    e.set_cached_init(
        "/proj",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["claude".into(), "Plan".into(), "Explore".into()],
        },
    );
    assert_eq!(e.resolve_agent(&s.id, "Plan"), "Plan"); // exact
    assert_eq!(e.resolve_agent(&s.id, "explore"), "Explore"); // case-insensitive
    assert_eq!(e.resolve_agent(&s.id, "plan"), "Plan"); // alias/known
    assert_eq!(e.resolve_agent(&s.id, "code-reviewer"), ""); // alias → default
    assert_eq!(e.resolve_agent(&s.id, "unknown-x"), ""); // unknown + known non-empty → default
}

#[test]
fn set_agent_no_change_is_noop() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    e.set_agent(&s.id, ""); // resolves empty → agent stays None
    assert_eq!(e.get_session(&s.id).unwrap().agent, None);
    e.set_cached_init(
        "/proj",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
        },
    );
    e.set_agent(&s.id, "plan");
    assert_eq!(e.get_session(&s.id).unwrap().agent.as_deref(), Some("Plan"));
    e.set_agent(&s.id, "Plan"); // unchanged
    assert_eq!(e.get_session(&s.id).unwrap().agent.as_deref(), Some("Plan"));
}

// ── init + model cache ──────────────────────────────────────────────

#[test]
fn init_cache_roundtrip() {
    let e = engine();
    assert!(e.cached_init("/d").is_none());
    e.set_cached_init(
        "/d",
        claude_cli::InitInfo {
            commands: vec!["compact".into()],
            agents: vec!["claude".into()],
        },
    );
    let got = e.cached_init("/d").unwrap();
    assert_eq!(got.commands, vec!["compact".to_string()]);
    assert_eq!(got.agents, vec!["claude".to_string()]);
}

#[test]
fn model_cache_roundtrip() {
    let e = engine();
    assert!(e.cached_models_any().is_none());
    e.set_cached_models(vec![claude_cli::ModelInfo {
        id: "m1".into(),
        display_name: "M1".into(),
        context_window: 100,
        max_output: 10,
    }]);
    let got = e.cached_models_any().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "m1");
}

// ── allowed tools / pending ─────────────────────────────────────────

#[test]
fn allowed_tools_dedup() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert!(!e.is_always_allowed(&s.id, "Bash"));
    e.add_allowed_tool(&s.id, "Bash");
    e.add_allowed_tool(&s.id, "Bash"); // dedup
    assert!(e.is_always_allowed(&s.id, "Bash"));
    assert_eq!(
        e.get_session(&s.id).unwrap().allowed_tools,
        vec!["Bash".to_string()]
    );
    assert!(!e.is_always_allowed("missing", "Bash"));
}

#[tokio::test]
async fn register_and_resolve_pending() {
    let e = engine();
    let rx = e.register_pending("id1");
    assert!(e.resolve_pending("id1", PendingReply::Reject));
    assert!(matches!(rx.await.unwrap(), PendingReply::Reject));
    // Unknown id → false.
    assert!(!e.resolve_pending("gone", PendingReply::Reject));
}

// ── emit / emit_system / save ───────────────────────────────────────

#[tokio::test]
async fn emit_delivers_to_subscriber() {
    let e = engine();
    let mut rx = e.subscribe();
    e.emit("/d", "custom", serde_json::json!({ "k": "v" }));
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.directory, "/d");
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "custom");
    assert_eq!(v["properties"]["k"], "v");
}

#[tokio::test]
async fn emit_system_levels_and_missing_session() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    for (level, variant) in [
        ("error", "error"),
        ("warn", "warning"),
        ("info", "notification"),
    ] {
        let mut rx = e.subscribe();
        e.emit_system(&s.id, level, "boom");
        let ev1 = rx.recv().await.unwrap();
        let v1: serde_json::Value = serde_json::from_str(&ev1.data).unwrap();
        assert_eq!(v1["type"], "message.updated");
        assert_eq!(v1["properties"]["info"]["variant"], variant);
        let ev2 = rx.recv().await.unwrap();
        let v2: serde_json::Value = serde_json::from_str(&ev2.data).unwrap();
        assert_eq!(v2["type"], "message.part.updated");
        assert_eq!(v2["properties"]["part"]["text"], "boom");
    }
    // Missing session → nothing emitted (no panic).
    e.emit_system("nope", "error", "x");
}

#[test]
fn save_and_load_roundtrip_with_persist() {
    let dir = std::env::temp_dir().join(format!("opman-reg-{}", rand::random::<u64>()));
    let path = dir.join("sessions.json");
    let e = Arc::new(ClaudeEngine::new(
        Some(path.clone()),
        (false, false, false, false),
    ));
    let s = e.create_session("/proj", "", "persisted"); // triggers save()
    assert!(path.exists());
    let reg = super::registry::Registry::load(&path);
    assert!(reg.sessions.contains_key(&s.id));
    let _ = std::fs::remove_dir_all(&dir);
}
