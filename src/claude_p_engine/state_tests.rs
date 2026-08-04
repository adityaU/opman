use super::*;
use crate::claude_engine::claude_cli::InitInfo;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

#[test]
fn create_get_and_list_sorted() {
    let e = engine();
    let a = e.create_session("d", "", "A");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = e.create_session("d", "", "B");
    let _other = e.create_session("other", "", "C");

    assert_eq!(e.get_session(&a.id).unwrap().title, "A");
    assert!(e.get_session("nope").is_none());

    let list = e.list_for_dir("d");
    assert_eq!(list.len(), 2);
    // Sorted by created desc → b before a.
    assert_eq!(list[0].id, b.id);
    assert_eq!(list[1].id, a.id);
    assert!(e.list_for_dir("empty").is_empty());
}

#[test]
fn busy_map_and_set_busy_edges() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // No edge when unchanged.
    assert!(!e.set_busy(&s.id, false));
    // idle→busy edge returns false (only busy→idle returns true).
    assert!(!e.set_busy(&s.id, true));
    assert_eq!(e.busy_map().get(&s.id), Some(&true));
    // busy→idle edge returns true.
    assert!(e.set_busy(&s.id, false));
    assert_eq!(e.busy_map().get(&s.id), Some(&false));
    // Missing session → false.
    assert!(!e.set_busy("missing", true));
}

#[test]
fn claude_uuid_lookup_and_resume() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert!(e.session_id_for_claude_uuid("").is_none());
    assert!(e.session_id_for_claude_uuid("uuid-x").is_none());
    assert!(e.resume_uuid(&s.id).is_none());

    e.set_claude_uuid(&s.id, "uuid-x");
    assert_eq!(
        e.session_id_for_claude_uuid("uuid-x").as_deref(),
        Some(s.id.as_str())
    );
    assert_eq!(e.resume_uuid(&s.id).as_deref(), Some("uuid-x"));

    // Setting the same uuid again is a no-op (change guard).
    e.set_claude_uuid(&s.id, "uuid-x");
    assert_eq!(e.resume_uuid(&s.id).as_deref(), Some("uuid-x"));

    e.forget_claude_uuid(&s.id);
    assert!(e.resume_uuid(&s.id).is_none());
}

#[test]
fn set_claude_uuid_missing_session_noop() {
    let e = engine();
    e.set_claude_uuid("nope", "u"); // change guard: get_session None → false, no panic
    assert!(e.get_session("nope").is_none());
}

#[test]
fn ensure_subagent_session_idempotent() {
    let e = engine();
    let parent = e.create_session("d", "", "P");
    e.ensure_subagent_session(&parent.id, "", "", "d"); // empty agent id → no-op
    assert!(e.get_session("").is_none());

    e.ensure_subagent_session(&parent.id, "agent-1", "", "d");
    let sub = e.get_session("agent-1").unwrap();
    assert!(sub.is_subagent);
    assert_eq!(sub.title, "Subagent");
    assert_eq!(sub.parent_id, parent.id);

    // Second call is idempotent (already present).
    e.ensure_subagent_session(&parent.id, "agent-1", "Renamed", "d");
    assert_eq!(e.get_session("agent-1").unwrap().title, "Subagent");

    // With an explicit title.
    e.ensure_subagent_session(&parent.id, "agent-2", "Custom", "d");
    assert_eq!(e.get_session("agent-2").unwrap().title, "Custom");
}

#[test]
fn set_title_and_rename_lock_logic() {
    let e = engine();
    let s = e.create_session("d", "", "orig");
    // Auto title change works while unlocked.
    e.set_title(&s.id, "auto1", false);
    assert_eq!(e.get_session(&s.id).unwrap().title, "auto1");
    // Same title → no change (early return).
    e.set_title(&s.id, "auto1", false);
    // Manual rename locks it.
    e.rename_session(&s.id, "manual");
    assert_eq!(e.get_session(&s.id).unwrap().title, "manual");
    assert!(e.get_session(&s.id).unwrap().title_locked);
    // Auto change ignored once locked.
    e.set_title(&s.id, "auto2", false);
    assert_eq!(e.get_session(&s.id).unwrap().title, "manual");
    // Manual can still override.
    e.set_title(&s.id, "manual2", true);
    assert_eq!(e.get_session(&s.id).unwrap().title, "manual2");
    // Missing session → no-op.
    e.set_title("nope", "x", true);
}

#[test]
fn set_model_and_agent() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_model(&s.id, ""); // empty → no-op
    assert!(e.get_session(&s.id).unwrap().model.is_none());
    e.set_model(&s.id, "m1");
    assert_eq!(e.get_session(&s.id).unwrap().model.as_deref(), Some("m1"));

    // With no cached init, an arbitrary agent name passes through.
    e.set_agent(&s.id, "custom-agent");
    assert_eq!(
        e.get_session(&s.id).unwrap().agent.as_deref(),
        Some("custom-agent")
    );
    // Empty agent clears (resolve_agent returns "" → None).
    e.set_agent(&s.id, "   ");
    assert!(e.get_session(&s.id).unwrap().agent.is_none());
}

#[test]
fn set_permission_mode_emits_toast() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    e.set_permission_mode(&s.id, "plan");
    assert_eq!(
        e.get_session(&s.id).unwrap().permission_mode.as_deref(),
        Some("plan")
    );
    // A toast event was emitted.
    let ev = rx.recv().now_or_never().unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "tui.toast.show");
}

#[test]
fn effective_mode_default_and_override() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert_eq!(e.effective_mode(&s.id), "bypassPermissions");
    e.set_permission_mode(&s.id, "acceptEdits");
    assert_eq!(e.effective_mode(&s.id), "acceptEdits");
    // Unknown session → engine default.
    assert_eq!(e.effective_mode("nope"), "bypassPermissions");
}

#[test]
fn allowed_tools_dedup_and_query() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert!(!e.is_always_allowed(&s.id, "Bash"));
    e.add_allowed_tool(&s.id, "Bash");
    e.add_allowed_tool(&s.id, "Bash"); // dedup
    assert!(e.is_always_allowed(&s.id, "Bash"));
    assert_eq!(e.get_session(&s.id).unwrap().allowed_tools.len(), 1);
    assert!(!e.is_always_allowed("nope", "Bash"));
}

#[test]
fn pending_register_and_resolve() {
    let e = engine();
    let _rx = e.register_pending("req-1");
    // Resolving a registered id succeeds.
    assert!(e.resolve_pending("req-1", PendingReply::Permission("once".into())));
    // Unknown id → false.
    assert!(!e.resolve_pending("req-unknown", PendingReply::Reject));
}

#[test]
fn pending_resolve_dropped_receiver() {
    let e = engine();
    {
        let _rx = e.register_pending("req-2");
    } // rx dropped here
      // send fails because the receiver is gone → false.
    assert!(!e.resolve_pending("req-2", PendingReply::Reject));
}

#[test]
fn init_cache_get_set() {
    let e = engine();
    assert!(e.cached_init("d").is_none());
    e.set_cached_init(
        "d",
        InitInfo {
            commands: vec!["c".into()],
            agents: vec!["Plan".into()],
        },
    );
    let got = e.cached_init("d").unwrap();
    assert_eq!(got.commands, vec!["c".to_string()]);
    assert_eq!(got.agents, vec!["Plan".to_string()]);
}

#[test]
fn resolve_agent_branches() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // Empty → "".
    assert_eq!(e.resolve_agent(&s.id, "  "), "");
    // No cache: "plan" → "Plan".
    assert_eq!(e.resolve_agent(&s.id, "plan"), "Plan");
    // No cache: build/reviewer families → "".
    assert_eq!(e.resolve_agent(&s.id, "build"), "");
    assert_eq!(e.resolve_agent(&s.id, "code-reviewer"), "");
    assert_eq!(e.resolve_agent(&s.id, "reviewer"), "");
    // No cache: arbitrary passes through.
    assert_eq!(e.resolve_agent(&s.id, "whatever"), "whatever");

    // With cache: exact (case-insensitive) match returns the real name.
    e.set_cached_init(
        "d",
        InitInfo {
            commands: vec![],
            agents: vec!["Researcher".into()],
        },
    );
    assert_eq!(e.resolve_agent(&s.id, "researcher"), "Researcher");
    // Known non-empty, unknown name → "".
    assert_eq!(e.resolve_agent(&s.id, "ghost"), "");
    // Known non-empty without a Plan agent, asking plan → "".
    assert_eq!(e.resolve_agent(&s.id, "plan"), "");

    // With a Plan in cache, "plan" resolves to it.
    e.set_cached_init(
        "d",
        InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
        },
    );
    assert_eq!(e.resolve_agent(&s.id, "plan"), "Plan");
}

#[tokio::test]
async fn delete_session_removes_and_emits() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    e.delete_session(&s.id).await;
    assert!(e.get_session(&s.id).is_none());
    // A session.deleted event was emitted.
    let mut saw_delete = false;
    while let Some(Ok(ev)) = rx.recv().now_or_never() {
        let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
        if v["type"] == "session.deleted" {
            saw_delete = true;
        }
    }
    assert!(saw_delete);
    // Deleting a missing session is a no-op.
    e.delete_session("nope").await;
}

use futures::FutureExt;
