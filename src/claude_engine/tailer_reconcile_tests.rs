//! Remaining `tick_status_poller` reconciliation branches not covered by
//! `tailer_poller_tests.rs`: a `failed` state for a turn we never saw running (no
//! notification), a plain busy→idle edge with no queued follow-up, and recovery of a
//! session that was absent for one poll then reappears busy.
use super::*;
use std::collections::HashSet;

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

fn set_uuid(e: &Arc<ClaudeEngine>, sid: &str, uuid: &str) {
    e.reg
        .lock()
        .unwrap()
        .sessions
        .get_mut(sid)
        .unwrap()
        .claude_session_id = Some(uuid.to_string());
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
async fn poller_failed_state_not_seen_does_not_notify() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    let (mut absent, mut seen, mut notified) = maps();
    // `seen_busy` is empty — we never observed this turn running (e.g. a pre-existing
    // failed agent from an earlier opman run), so no error bubble is surfaced.
    let mut rx = e.subscribe();
    tick_status_poller(
        &e,
        &[agent("u1", Some("failed"))],
        &mut absent,
        &mut seen,
        &mut notified,
    );
    assert!(!notified.contains("u1"));
    // No error bubble is surfaced: if any event is emitted it is at most the
    // (idle) session.status flip, never a message.updated error.
    if let Ok(ev) = rx.try_recv() {
        let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
        assert_ne!(v["type"], "message.updated");
    }
}

#[tokio::test]
async fn poller_busy_to_idle_edge_without_queue() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    let (mut absent, mut seen, mut notified) = maps();
    // Agent reports done and there is no queued follow-up → flips idle, no re-dispatch.
    tick_status_poller(
        &e,
        &[agent("u1", Some("done"))],
        &mut absent,
        &mut seen,
        &mut notified,
    );
    assert!(!e.get_session(&s.id).unwrap().busy);
    assert!(!e.is_dispatching(&s.id)); // nothing queued → no spawn_turn
}

#[tokio::test]
async fn poller_reappears_busy_clears_absence() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    set_uuid(&e, &s.id, "u1");
    set_busy_flag(&e, &s.id);
    let (mut absent, mut seen, mut notified) = maps();
    // One absent poll bumps the debounce counter without flipping idle.
    tick_status_poller(&e, &[], &mut absent, &mut seen, &mut notified);
    assert_eq!(absent.get("u1"), Some(&1));
    // The agent reappears working → the absence counter is cleared and it stays busy.
    tick_status_poller(
        &e,
        &[agent("u1", Some("working"))],
        &mut absent,
        &mut seen,
        &mut notified,
    );
    assert!(!absent.contains_key("u1"));
    assert!(e.get_session(&s.id).unwrap().busy);
    assert!(seen.contains("u1"));
}
