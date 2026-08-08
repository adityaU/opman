//! Generated coverage tests for `mod.rs` — session lifecycle, turns, queue.
use super::*;

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ))
}

fn agent(session_id: &str, cwd: &str) -> claude_cli::AgentInfo {
    claude_cli::AgentInfo {
        id: "sh".into(),
        session_id: session_id.into(),
        cwd: cwd.into(),
        kind: String::new(),
        state: None,
        status: None,
        name: String::new(),
        started_at: 0,
    }
}

// ── record_turn ─────────────────────────────────────────────────────

#[tokio::test]
async fn record_turn_updates_and_ignores_empty_uuid() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    e.record_turn(&s.id, "short1".into(), "uuid1".into(), Some("opus".into()));
    let entry = e.get_session(&s.id).unwrap();
    assert!(entry.busy);
    assert_eq!(entry.short_id.as_deref(), Some("short1"));
    assert_eq!(entry.claude_session_id.as_deref(), Some("uuid1"));
    assert_eq!(entry.lineage.last().map(|x| x.as_str()), Some("uuid1"));
    assert_eq!(entry.model.as_deref(), Some("opus"));

    // Empty uuid keeps the previous one; None model keeps the previous model.
    e.record_turn(&s.id, "short2".into(), String::new(), None);
    let entry = e.get_session(&s.id).unwrap();
    assert_eq!(entry.claude_session_id.as_deref(), Some("uuid1"));
    assert_eq!(entry.short_id.as_deref(), Some("short2"));
    assert_eq!(entry.model.as_deref(), Some("opus"));
    assert_eq!(entry.lineage.len(), 1); // uuid1 not duplicated

    // Missing session → no-op.
    e.record_turn("nope", "s".into(), "u".into(), None);
}

// ── spawn_turn (synchronous guards) ─────────────────────────────────

#[tokio::test]
async fn spawn_turn_guards_and_marks_busy() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    // Empty text → early return.
    e.spawn_turn(s.id.clone(), "   ".into());
    assert!(!e.is_dispatching(&s.id));
    assert!(!e.get_session(&s.id).unwrap().busy);
    // Missing session → early return.
    e.spawn_turn("nope".into(), "hi".into());
    // Real dispatch synchronously marks dispatching + busy (the spawned claude turn
    // then fails in the background because no claude binary is present).
    e.spawn_turn(s.id.clone(), "do work".into());
    assert!(e.is_dispatching(&s.id));
    assert!(e.get_session(&s.id).unwrap().busy);
}

// ── set_title ───────────────────────────────────────────────────────

#[test]
fn set_title_auto_manual_lock() {
    let e = engine();
    let s = e.create_session("/d", "", "original");
    e.set_title(&s.id, "auto1", false);
    assert_eq!(e.get_session(&s.id).unwrap().title, "auto1");
    e.set_title(&s.id, "Manual", true);
    assert!(e.get_session(&s.id).unwrap().title_locked);
    // Auto title cannot override a locked (manual) title.
    e.set_title(&s.id, "auto2", false);
    assert_eq!(e.get_session(&s.id).unwrap().title, "Manual");
    // Re-setting the same manual title is a no-op.
    e.set_title(&s.id, "Manual", true);
    assert_eq!(e.get_session(&s.id).unwrap().title, "Manual");
    // Missing session → no-op.
    e.set_title("nope", "x", true);
}

// ── remove_session tombstones ───────────────────────────────────────

#[test]
fn remove_session_tombstones_lineage() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    {
        let mut g = e.reg.lock().unwrap();
        let ent = g.sessions.get_mut(&s.id).unwrap();
        ent.claude_session_id = Some("u1".into());
        ent.lineage = vec!["u0".into(), "u1".into()];
    }
    e.remove_session(&s.id);
    assert!(e.get_session(&s.id).is_none());
    {
        let g = e.reg.lock().unwrap();
        assert!(g.deleted.contains("u1"));
        assert!(g.deleted.contains("u0"));
    }
    // Removing a missing session is a no-op.
    e.remove_session("nope");
}

// ── ensure_subagent_session edges ───────────────────────────────────

#[test]
fn ensure_subagent_edges() {
    let e = engine();
    let p = e.create_session("/d", "", "p");
    // Empty agent id → nothing registered.
    e.ensure_subagent_session(&p.id, "", "t", "/d");
    // Deleted agent id is not resurrected.
    e.reg.lock().unwrap().deleted.insert("agent_del".into());
    e.ensure_subagent_session(&p.id, "agent_del", "t", "/d");
    assert!(e.get_session("agent_del").is_none());
    // Empty title defaults to "Subagent".
    e.ensure_subagent_session(&p.id, "agent_e", "", "/d");
    assert_eq!(e.get_session("agent_e").unwrap().title, "Subagent");
}

// ── session_id_for_claude_uuid ──────────────────────────────────────

#[test]
fn session_lookup_by_uuid_and_lineage() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    {
        let mut g = e.reg.lock().unwrap();
        let ent = g.sessions.get_mut(&s.id).unwrap();
        ent.claude_session_id = Some("uuidA".into());
        ent.lineage = vec!["uuidL".into()];
    }
    assert_eq!(
        e.session_id_for_claude_uuid("uuidA").as_deref(),
        Some(s.id.as_str())
    );
    assert_eq!(
        e.session_id_for_claude_uuid("uuidL").as_deref(),
        Some(s.id.as_str())
    );
    assert!(e.session_id_for_claude_uuid("").is_none());
    assert!(e.session_id_for_claude_uuid("missing").is_none());
}

// ── subagent_pending / dispatching ──────────────────────────────────

#[test]
fn subagent_pending_and_dispatching_flags() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert!(!e.subagent_pending(&s.id));
    e.set_subagent_pending(&s.id, true);
    assert!(e.subagent_pending(&s.id));
    e.set_subagent_pending("nope", true); // no-op
    assert!(!e.subagent_pending("nope"));
    assert!(!e.is_dispatching(&s.id));
    assert!(!e.is_occupied(&s.id));
}

// ── aborting / settling ─────────────────────────────────────────────

#[test]
fn abort_settling_lifecycle() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert!(!e.abort_settling(&s.id, true)); // not marked
    e.mark_aborting(&s.id);
    assert!(e.abort_settling(&s.id, true)); // busy + fresh → settling
    assert!(!e.abort_settling(&s.id, false)); // agent idle → resolves + clears
    assert!(!e.abort_settling(&s.id, true)); // already cleared
    e.mark_aborting(&s.id);
    e.clear_aborting(&s.id);
    assert!(!e.abort_settling(&s.id, true));
}

// ── queue management ────────────────────────────────────────────────

#[test]
fn remove_pending_bounds_and_empty() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert!(!e.remove_pending(&s.id, 0)); // no queue
    e.enqueue_prompt(&s.id, "a".into());
    e.enqueue_prompt(&s.id, "b".into());
    assert!(!e.remove_pending(&s.id, 5)); // out of range
    assert!(e.remove_pending(&s.id, 0)); // removes "a"
    assert_eq!(e.pending_list(&s.id), vec!["b".to_string()]);
    assert!(e.remove_pending(&s.id, 0)); // removes last → drops the entry
    assert!(e.pending_list(&s.id).is_empty());
    assert!(!e.remove_pending(&s.id, 0)); // entry now gone
}

#[tokio::test]
async fn emit_queue_changed_emits_and_skips_missing() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    e.enqueue_prompt(&s.id, "x".into());
    let mut rx = e.subscribe();
    e.emit_queue_changed(&s.id);
    let ev = rx.recv().await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "session.queue");
    assert_eq!(v["properties"]["pending"][0], "x");
    // Missing session → no emit (no panic).
    e.emit_queue_changed("nope");
}

// ── busy_map / set_busy transitions ─────────────────────────────────

#[test]
fn busy_map_and_set_busy_edges() {
    let e = engine();
    let a = e.create_session("/d", "", "a");
    assert_eq!(e.busy_map().get(&a.id), Some(&false));
    assert!(!e.set_busy(&a.id, true)); // busy edge is not the idle edge
    assert_eq!(e.busy_map().get(&a.id), Some(&true));
    assert!(e.set_busy(&a.id, false)); // idle edge
    assert!(!e.set_busy(&a.id, false)); // no change
    assert!(!e.set_busy("nope", true)); // missing session
}

// ── hook_settings / build_opts ──────────────────────────────────────

#[test]
fn hook_settings_shape() {
    let e = engine();
    let js: serde_json::Value = serde_json::from_str(&e.hook_settings()).unwrap();
    assert!(js["hooks"]["PreToolUse"].is_array());
    assert_eq!(js["worktree"]["bgIsolation"], "none");
    let hook = &js["hooks"]["PreToolUse"][0]["hooks"][0];
    assert!(hook["command"].as_str().unwrap().contains("claude-hook"));
    assert_eq!(hook["timeout"], 3600);
}

#[test]
fn build_opts_assembles_turn_options() {
    let e = engine();
    e.set_url("http://engine");
    let s = e.create_session("/d", "", "t");
    e.set_cached_init(
        "/d",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
            ..Default::default()
        },
    );
    e.set_model(&s.id, "opus");
    e.set_agent(&s.id, "plan");
    e.set_effort(&s.id, "high");
    e.set_permission_mode(&s.id, "acceptEdits");
    let opts = e.build_opts(&s.id, "/d");
    assert_eq!(opts.model.as_deref(), Some("opus"));
    assert_eq!(opts.agent.as_deref(), Some("Plan"));
    assert_eq!(opts.effort.as_deref(), Some("high"));
    assert_eq!(opts.permission_mode, "acceptEdits");
    assert!(!opts.settings_json.is_empty());
    assert_eq!(opts.engine_url, "http://engine");
    assert_eq!(opts.session_env_id, s.id);
    assert_eq!(opts.mcp_config, "");
}

// ── import_agents skip branches ─────────────────────────────────────

#[test]
fn import_agents_skips_all_non_importable() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    {
        let mut g = e.reg.lock().unwrap();
        g.sessions.get_mut(&s.id).unwrap().claude_session_id = Some("uuid-known".into());
        g.deleted.insert("uuid-del".into());
    }
    let before = e.list_for_dir("/d").len();
    e.import_agents(
        "/d",
        vec![
            agent("", "/d"),           // empty session id
            agent("uuid-x", "/other"), // cwd mismatch
            agent("uuid-known", "/d"), // already known
            agent("uuid-del", "/d"),   // tombstoned
        ],
    );
    assert_eq!(e.list_for_dir("/d").len(), before); // nothing imported
    e.import_agents("/d", vec![]); // empty input is a no-op
}

// ── global accessors ────────────────────────────────────────────────

#[test]
fn global_accessors_are_callable() {
    let _ = super::engine();
    let _ = super::short_id_for_session("whatever");
}
