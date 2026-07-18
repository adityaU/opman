//! Single-pass drive tests for `tick_status_poller`, exercised with crafted
//! `AgentInfo` lists and the poller's carried debounce/notify state — covering the
//! dispatch/abort/absent/failed/idle-flush reconciliation branches without a live
//! `claude` or the 2s polling loop.
use super::*;
use std::collections::HashSet;

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

fn set_uuid(e: &Arc<ClaudeEngine>, sid: &str, uuid: &str) {
    e.reg.lock().unwrap().sessions.get_mut(sid).unwrap().claude_session_id = Some(uuid.to_string());
}

fn set_busy_flag(e: &Arc<ClaudeEngine>, sid: &str) {
    e.reg.lock().unwrap().sessions.get_mut(sid).unwrap().busy = true;
}

fn agent(session_id: &str, state: Option<&str>) -> claude_cli::AgentInfo {
    claude_cli::AgentInfo {
        id: "sh".into(),
        session_id: session_id.into(),
        cwd: "/d".into(),
        kind: String::new(),
        state: state.map(String::from),
        status: None,
        name: String::new(),
        started_at: 0,
    }
}

fn maps() -> (HashMap<String, u32>, HashSet<String>, HashSet<String>) {
    (HashMap::new(), HashSet::new(), HashSet::new())
}

#[tokio::test]
async fn poller_marks_busy_and_records_seen() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    let (mut absent, mut seen, mut notified) = maps();
    tick_status_poller(&e, &[agent("u1", Some("working"))], &mut absent, &mut seen, &mut notified);
    assert!(e.get_session(&s.id).unwrap().busy);
    assert!(seen.contains("u1"));
    assert!(e.tailers.lock().unwrap().contains(&s.id)); // busy → ensure_tailer
}

#[tokio::test]
async fn poller_skips_sessions_without_uuid() {
    let e = engine();
    let s = e.create_session("/d", "", "t"); // no claude_session_id
    let (mut a, mut sb, mut nf) = maps();
    tick_status_poller(&e, &[], &mut a, &mut sb, &mut nf);
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn poller_skips_dispatching_sessions() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    e.dispatching.lock().unwrap().insert(s.id.clone());
    let (mut a, mut sb, mut nf) = maps();
    // Agent reports done, but the dispatch guard means the poller must not touch it.
    tick_status_poller(&e, &[agent("u1", Some("done"))], &mut a, &mut sb, &mut nf);
    assert!(e.get_session(&s.id).unwrap().busy); // left busy (skipped)
}

#[tokio::test]
async fn poller_abort_settling_forces_idle() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    e.mark_aborting(&s.id);
    let (mut absent, mut sb, mut nf) = maps();
    absent.insert("u1".into(), 2);
    // Agent still working → raw_busy true → settling window active → forced idle.
    tick_status_poller(&e, &[agent("u1", Some("working"))], &mut absent, &mut sb, &mut nf);
    assert!(!e.get_session(&s.id).unwrap().busy);
    assert!(!absent.contains_key("u1")); // debounce cleared
}

#[tokio::test]
async fn poller_absent_debounces_then_flips_idle() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    let (mut absent, mut sb, mut nf) = maps();
    // First two absences only increment the debounce counter — session stays busy.
    tick_status_poller(&e, &[], &mut absent, &mut sb, &mut nf);
    assert_eq!(absent.get("u1"), Some(&1));
    assert!(e.get_session(&s.id).unwrap().busy);
    tick_status_poller(&e, &[], &mut absent, &mut sb, &mut nf);
    assert_eq!(absent.get("u1"), Some(&2));
    assert!(e.get_session(&s.id).unwrap().busy);
    // Third absence reaches the threshold → flips idle.
    tick_status_poller(&e, &[], &mut absent, &mut sb, &mut nf);
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn poller_subagent_pending_keeps_busy_past_absence() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    e.set_subagent_pending(&s.id, true);
    let (mut absent, mut sb, mut nf) = maps();
    absent.insert("u1".into(), 2); // next absence hits the threshold
    tick_status_poller(&e, &[], &mut absent, &mut sb, &mut nf);
    // Agent gone, but an in-flight subagent keeps the session busy.
    assert!(e.get_session(&s.id).unwrap().busy);
    assert!(sb.contains("u1"));
    assert!(e.tailers.lock().unwrap().contains(&s.id));
}

#[tokio::test]
async fn poller_failed_state_notifies_once() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    let (mut absent, mut sb, mut nf) = maps();
    sb.insert("u1".into()); // we saw this turn running earlier
    let mut rx = e.subscribe();
    tick_status_poller(&e, &[agent("u1", Some("failed"))], &mut absent, &mut sb, &mut nf);
    assert!(nf.contains("u1"));
    let ev = rx.recv().await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "message.updated");
    assert_eq!(v["properties"]["info"]["level"], "error");
    // A second pass must not re-notify (idempotent) — no panic, still just tracked once.
    tick_status_poller(&e, &[agent("u1", Some("failed"))], &mut absent, &mut sb, &mut nf);
    assert!(nf.contains("u1"));
}

#[tokio::test]
async fn poller_flushes_queue_on_idle_edge() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    e.enqueue_prompt(&s.id, "resume me".into());
    let (mut absent, mut sb, mut nf) = maps();
    // Agent finished → busy→idle edge → the queued prompt is taken and re-dispatched.
    tick_status_poller(&e, &[agent("u1", Some("done"))], &mut absent, &mut sb, &mut nf);
    assert!(e.take_pending(&s.id).is_none()); // queue drained by the flush
    assert!(e.is_dispatching(&s.id)); // spawn_turn re-armed the dispatch guard
}

#[tokio::test]
async fn poller_ignores_subagent_rows() {
    let e = engine();
    let p = e.create_session("/d", "", "p");
    e.ensure_subagent_session(&p.id, "agent_x", "sub", "/d");
    e.reg.lock().unwrap().sessions.get_mut("agent_x").unwrap().claude_session_id = Some("subu".into());
    let (mut a, mut sb, mut nf) = maps();
    tick_status_poller(&e, &[agent("subu", Some("working"))], &mut a, &mut sb, &mut nf);
    assert!(!e.get_session("agent_x").unwrap().busy); // filtered out of reconciliation
    assert!(!sb.contains("subu"));
}
