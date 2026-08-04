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

#[test]
fn message_hash_stable_and_sensitive() {
    let a = jsonl::MsgOut {
        info: json!({ "id": "m1" }),
        parts: vec![json!({ "x": 1 })],
    };
    let b = jsonl::MsgOut {
        info: json!({ "id": "m1" }),
        parts: vec![json!({ "x": 1 })],
    };
    let c = jsonl::MsgOut {
        info: json!({ "id": "m1" }),
        parts: vec![json!({ "x": 2 })],
    };
    let d = jsonl::MsgOut {
        info: json!({ "id": "m2" }),
        parts: vec![json!({ "x": 1 })],
    };
    assert_eq!(message_hash(&a), message_hash(&b));
    assert_ne!(message_hash(&a), message_hash(&c));
    assert_ne!(message_hash(&a), message_hash(&d));
}

#[test]
fn claude_bin_defaults_nonempty() {
    assert!(!claude_bin().is_empty());
}

#[tokio::test]
async fn send_unknown_session_is_noop() {
    let e = engine();
    send(e.clone(), "nope".to_string(), "hi".to_string()).await;
    assert!(e.procs.0.lock().await.is_empty());
}

#[tokio::test]
async fn send_spawn_failure_when_cwd_missing() {
    let e = engine();
    let n: u128 = rand::random();
    let s = e.create_session(&format!("/nonexistent/opman_{n:032x}"), "", "A");
    send(e.clone(), s.id.clone(), "hi".to_string()).await;
    // Spawn failed → no process registered, session not busy.
    assert!(e.procs.0.lock().await.is_empty());
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn abort_without_process_clears_busy() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_busy(&s.id, true);
    abort(e.clone(), &s.id).await;
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn abort_kills_live_process() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    e.procs
        .0
        .lock()
        .await
        .insert(s.id.clone(), Proc { stdin, child });
    abort(e.clone(), &s.id).await;
    assert!(e.procs.0.lock().await.is_empty());
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn reader_full_lifecycle_clean_result() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let uuid = format!("opman-test-uuid-{:x}", rand::random::<u64>());
    let script = format!(
        "printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{uuid}\"}}' '' 'not json' '{{\"type\":\"other\"}}' '{{\"type\":\"assistant\"}}' '{{\"type\":\"user\"}}' '{{\"type\":\"result\",\"subtype\":\"success\"}}'"
    );
    run_reader(e.clone(), &s.id, &script, false).await;
    // init recorded the claude uuid; the turn ended cleanly (not busy).
    assert_eq!(
        e.get_session(&s.id).unwrap().claude_uuid.as_deref(),
        Some(uuid.as_str())
    );
    assert!(!e.get_session(&s.id).unwrap().busy);
    assert!(e.procs.0.lock().await.is_empty());
}

#[tokio::test]
async fn reader_result_is_error_surfaces_detail() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    let uuid = format!("opman-test-uuid-{:x}", rand::random::<u64>());
    let script = format!(
        "printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{uuid}\"}}' '{{\"type\":\"result\",\"is_error\":true,\"result\":\"boom detail\"}}'"
    );
    run_reader(e.clone(), &s.id, &script, false).await;
    let events = drain(&mut rx);
    assert!(
        events.iter().any(|d| d.contains("boom detail")),
        "error detail surfaced"
    );
}

#[tokio::test]
async fn reader_result_nonsuccess_subtype_default_detail() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    let uuid = format!("opman-test-uuid-{:x}", rand::random::<u64>());
    let script = format!(
        "printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{uuid}\"}}' '{{\"type\":\"result\",\"subtype\":\"error_max_turns\"}}'"
    );
    run_reader(e.clone(), &s.id, &script, false).await;
    let events = drain(&mut rx);
    assert!(events.iter().any(|d| d.contains("error_max_turns")));
}

#[tokio::test]
async fn reader_crash_without_result_emits_interrupted() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    let uuid = format!("opman-test-uuid-{:x}", rand::random::<u64>());
    let script = format!(
        "printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{uuid}\"}}' '{{\"type\":\"assistant\"}}'"
    );
    run_reader(e.clone(), &s.id, &script, false).await;
    let events = drain(&mut rx);
    assert!(events.iter().any(|d| d.contains("exited unexpectedly")));
}

#[tokio::test]
async fn reader_stale_resume_forgets_uuid() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "stale-uuid");
    // attempted_resume=true, but the child produced no init event.
    run_reader(e.clone(), &s.id, "true", true).await;
    assert!(e.get_session(&s.id).unwrap().claude_uuid.is_none());
}

#[tokio::test]
async fn emit_system_variants_and_missing_session() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // Unknown session → no events.
    emit_system(&e, "nope", "error", "x");
    let mut rx = e.subscribe();
    emit_system(&e, &s.id, "warning", "careful");
    let events = drain(&mut rx);
    // message.updated + message.part.updated.
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|d| d.contains("\"variant\":\"warning\"")));
    assert!(events.iter().any(|d| d.contains("careful")));

    // "warn" maps to warning, unknown level → notification.
    let mut rx2 = e.subscribe();
    emit_system(&e, &s.id, "warn", "w");
    emit_system(&e, &s.id, "info", "i");
    let ev2 = drain(&mut rx2);
    assert!(ev2.iter().any(|d| d.contains("\"variant\":\"warning\"")));
    assert!(ev2
        .iter()
        .any(|d| d.contains("\"variant\":\"notification\"")));
}

#[tokio::test]
async fn reparse_emit_early_returns() {
    let e = engine();
    // Unknown session → returns without panic.
    reparse_emit(&e, "nope").await;
    // Session without a claude uuid → returns.
    let s = e.create_session("d", "", "A");
    reparse_emit(&e, &s.id).await;
    assert!(e.get_session(&s.id).unwrap().claude_uuid.is_none());
}
