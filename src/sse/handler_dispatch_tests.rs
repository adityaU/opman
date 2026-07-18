//! Generated tests for `handle_sse_data`: every event-type match arm is driven
//! with crafted JSON and the emitted `BackgroundEvent` (if any) is asserted.
//!
//! `BackgroundEvent` does not implement `PartialEq` (it holds non-comparable
//! payloads like `PtyInstance`), so we match variants with pattern matches.

use super::*;
use crate::app::BackgroundEvent;
use tokio::sync::mpsc;

fn ch() -> (
    mpsc::UnboundedSender<BackgroundEvent>,
    mpsc::UnboundedReceiver<BackgroundEvent>,
) {
    mpsc::unbounded_channel()
}

/// Feed `data` to the handler and collect all emitted events.
fn run(data: &str) -> (Result<()>, Vec<BackgroundEvent>) {
    let (tx, mut rx) = ch();
    let res = handle_sse_data(&tx, 3, data);
    drop(tx);
    let mut out = Vec::new();
    while let Ok(e) = rx.try_recv() {
        out.push(e);
    }
    (res, out)
}

// ── invalid input ───────────────────────────────────────────────────

#[test]
fn invalid_json_returns_err() {
    let (res, evs) = run("not json at all");
    assert!(res.is_err());
    assert!(evs.is_empty());
}

#[test]
fn missing_properties_returns_err() {
    // Missing the required `properties` field on the outer SseEvent.
    let (res, _) = run(r#"{"type":"session.created"}"#);
    assert!(res.is_err());
}

#[test]
fn session_created_bad_info_propagates_err() {
    // `session.created` with a non-object `info` fails from_value → Err.
    let (res, _) = run(r#"{"type":"session.created","properties":{"info":42}}"#);
    assert!(res.is_err());
}

// ── session lifecycle ───────────────────────────────────────────────

#[test]
fn session_created_emits_event() {
    let (res, evs) = run(
        r#"{"type":"session.created","properties":{"info":{"id":"s1","title":"T","directory":"/p"}}}"#,
    );
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionCreated { project_idx: 3, session }] if session.id == "s1"
    ));
}

#[test]
fn session_updated_emits_event() {
    let (res, evs) = run(
        r#"{"type":"session.updated","properties":{"info":{"id":"s2","directory":"/p"}}}"#,
    );
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionUpdated { session, .. }] if session.id == "s2"
    ));
}

#[test]
fn session_deleted_emits_event() {
    let (res, evs) = run(r#"{"type":"session.deleted","properties":{"sessionID":"s3"}}"#);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionDeleted { session_id, .. }] if session_id == "s3"
    ));
}

#[test]
fn session_idle_event_emits_idle() {
    let (res, evs) = run(r#"{"type":"session.idle","properties":{"sessionID":"s4"}}"#);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionIdle { session_id, .. }] if session_id == "s4"
    ));
}

// ── session.status ──────────────────────────────────────────────────

#[test]
fn session_status_busy_emits_busy() {
    let (res, evs) = run(
        r#"{"type":"session.status","properties":{"sessionID":"b","status":{"type":"busy"}}}"#,
    );
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionBusy { session_id }] if session_id == "b"
    ));
}

#[test]
fn session_status_retry_emits_busy() {
    let (_res, evs) = run(
        r#"{"type":"session.status","properties":{"sessionID":"r","status":{"type":"retry"}}}"#,
    );
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionBusy { .. }]
    ));
}

#[test]
fn session_status_idle_emits_idle() {
    let (_res, evs) = run(
        r#"{"type":"session.status","properties":{"sessionID":"i","status":{"type":"idle"}}}"#,
    );
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionIdle { session_id, .. }] if session_id == "i"
    ));
}

#[test]
fn session_status_unknown_type_emits_nothing() {
    let (res, evs) = run(
        r#"{"type":"session.status","properties":{"sessionID":"u","status":{"type":"compacting"}}}"#,
    );
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── connectionless / no-op arms ─────────────────────────────────────

#[test]
fn server_connected_emits_nothing() {
    let (res, evs) = run(r#"{"type":"server.connected","properties":{}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

#[test]
fn unknown_event_type_emits_nothing() {
    let (res, evs) = run(r#"{"type":"something.random","properties":{"a":1}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── file.edited ─────────────────────────────────────────────────────

#[test]
fn file_edited_with_file_emits_event() {
    let (res, evs) = run(r#"{"type":"file.edited","properties":{"file":"/p/a.rs"}}"#);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseFileEdited { file_path, .. }] if file_path == "/p/a.rs"
    ));
}

#[test]
fn file_edited_without_file_emits_nothing() {
    let (res, evs) = run(r#"{"type":"file.edited","properties":{"nofile":true}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── todo.updated ────────────────────────────────────────────────────

#[test]
fn todo_updated_emits_event() {
    let data = r#"{"type":"todo.updated","properties":{"sessionID":"t1","todos":[
        {"content":"do it","status":"pending","priority":"high"}]}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseTodoUpdated { session_id, todos }]
            if session_id == "t1" && todos.len() == 1
    ));
}

#[test]
fn todo_updated_missing_session_emits_nothing() {
    let (res, evs) = run(r#"{"type":"todo.updated","properties":{"todos":[]}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

#[test]
fn todo_updated_malformed_todos_emits_nothing() {
    // sessionID present but todos aren't a Vec<TodoItem> → inner from_value fails.
    let data = r#"{"type":"todo.updated","properties":{"sessionID":"t2","todos":"nope"}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

#[test]
fn todo_updated_absent_todos_key_emits_nothing() {
    // sessionID present but `todos` key absent → unwrap_or_default() yields
    // Value::Null, which fails to deserialize into Vec<TodoItem> → no event.
    let data = r#"{"type":"todo.updated","properties":{"sessionID":"t3"}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── permission.asked / question.asked ───────────────────────────────

#[test]
fn permission_asked_valid_emits_event() {
    let data = r#"{"type":"permission.asked","properties":{"id":"p1","sessionID":"s","permission":"edit"}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SsePermissionAsked { request, .. }] if request.id == "p1"
    ));
}

#[test]
fn permission_asked_invalid_emits_nothing() {
    // Missing required `id` field → from_value err → warn, no event, Ok overall.
    let (res, evs) = run(r#"{"type":"permission.asked","properties":{"sessionID":"s"}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

#[test]
fn question_asked_valid_emits_event() {
    let data = r#"{"type":"question.asked","properties":{"id":"q1","sessionID":"s","questions":[]}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseQuestionAsked { request, .. }] if request.id == "q1"
    ));
}

#[test]
fn question_asked_invalid_emits_nothing() {
    let (res, evs) = run(r#"{"type":"question.asked","properties":{"sessionID":"s"}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── session.error ───────────────────────────────────────────────────

#[test]
fn session_error_with_id_emits_event() {
    let (res, evs) = run(r#"{"type":"session.error","properties":{"sessionID":"e1"}}"#);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseSessionError { session_id }] if session_id == "e1"
    ));
}

#[test]
fn session_error_empty_id_emits_nothing() {
    let (res, evs) = run(r#"{"type":"session.error","properties":{"sessionID":""}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

#[test]
fn session_error_missing_id_emits_nothing() {
    let (res, evs) = run(r#"{"type":"session.error","properties":{}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}

// ── message.updated (via the `_` catch-all arm) ─────────────────────

#[test]
fn message_updated_emits_event() {
    let data = r#"{"type":"message.updated","properties":{"info":{"sessionID":"m1","cost":0.25,
        "tokens":{"input":10,"output":20,"reasoning":3,"cache":{"read":4,"write":5}}}}}"#;
    let (res, evs) = run(data);
    assert!(res.is_ok());
    assert!(matches!(
        evs.as_slice(),
        [BackgroundEvent::SseMessageUpdated { session_id, input_tokens, output_tokens, .. }]
            if session_id == "m1" && *input_tokens == 10 && *output_tokens == 20
    ));
}

#[test]
fn message_updated_malformed_emits_nothing() {
    // `info` missing required sessionID → MessageUpdatedProps parse fails → no event.
    let (res, evs) = run(r#"{"type":"message.updated","properties":{"info":{"cost":1.0}}}"#);
    assert!(res.is_ok());
    assert!(evs.is_empty());
}
