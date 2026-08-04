//! Edge branches of the stream `reader`: a `system/init` with no `session_id`
//! (the `if let Some(uuid)` false arm), an `init`-then-clean-`result` run that
//! records `saw_init`/`clean_result` without surfacing an error, and a stream
//! that never emits `init` (so no "interrupted" bubble on EOF).

use super::*;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn drain(
    rx: &mut tokio::sync::broadcast::Receiver<crate::claude_engine::EngineEvent>,
) -> Vec<String> {
    let mut out = vec![];
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.data);
    }
    out
}

async fn run_reader(engine: Arc<ClaudePEngine>, sid: &str, script: &str, attempted: bool) {
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    reader(engine, sid.to_string(), stdout, attempted).await;
    let _ = child.wait().await;
}

#[tokio::test]
async fn reader_init_without_session_id_records_no_uuid() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    // init lacks `session_id` → saw_init true but the uuid is never set; a clean
    // result then closes the turn with no "interrupted" bubble.
    let script = "printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\"}' '{\"type\":\"result\",\"subtype\":\"success\"}'";
    run_reader(e.clone(), &s.id, script, false).await;
    assert!(e.get_session(&s.id).unwrap().claude_uuid.is_none());
    assert!(!e.get_session(&s.id).unwrap().busy);
    // No error/interrupted event (clean_result was set).
    let events = drain(&mut rx);
    assert!(events.iter().all(|d| !d.contains("exited unexpectedly")));
}

#[tokio::test]
async fn reader_system_non_init_subtype_ignored() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // A `system` event whose subtype is not `init` falls through the inner guard;
    // with no init at all, EOF must NOT surface an interrupted bubble.
    let mut rx = e.subscribe();
    let script = "printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"other\"}'";
    run_reader(e.clone(), &s.id, script, false).await;
    let events = drain(&mut rx);
    assert!(events.iter().all(|d| !d.contains("exited unexpectedly")));
    assert!(e.procs.0.lock().await.is_empty());
}

#[tokio::test]
async fn reader_no_init_not_attempted_keeps_uuid() {
    // attempted_resume=false and no init → the stale-uuid forget branch is skipped.
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "keep-me");
    run_reader(e.clone(), &s.id, "true", false).await;
    assert_eq!(
        e.get_session(&s.id).unwrap().claude_uuid.as_deref(),
        Some("keep-me")
    );
}
