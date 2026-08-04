//! Generated tests (part 3) filling the remaining branches of
//! `handle_web_sse_event`: error-state cleared when a session goes busy, the
//! `file.edited` no-path branch, and reply events for unknown request ids.

use super::*;
use crate::web::types::WebEvent;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use tokio::sync::broadcast::Receiver;

fn handle_for(proj: &str) -> WebStateHandle {
    WebStateHandle::new_test_with_projects(vec![("p".to_string(), PathBuf::from(proj))])
}

fn drain(rx: &mut Receiver<WebEvent>) -> Vec<WebEvent> {
    let mut out = Vec::new();
    while let Ok(e) = rx.try_recv() {
        out.push(e);
    }
    out
}

#[tokio::test]
async fn busy_after_error_clears_error_and_emits_state_change() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // Put the session into an error state first (root, not active → also unseen).
    handle_web_sse_event(
        &h,
        r#"{"type":"session.error","properties":{"sessionID":"eb1","error":"boom"}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    // Now the session goes busy: the error entry is removed → StateChanged, and
    // it was not previously busy → SessionBusy.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"eb1","status":{"type":"busy"}}}"#,
        "/proj",
    )
    .await;
    let evs = drain(&mut rx);
    assert!(
        evs.iter().any(|e| matches!(e, WebEvent::StateChanged)),
        "error-clear should emit StateChanged"
    );
    assert!(evs
        .iter()
        .any(|e| matches!(e, WebEvent::SessionBusy { session_id } if session_id == "eb1")));
}

#[tokio::test]
async fn busy_when_already_busy_emits_no_duplicate() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"bb","status":{"type":"busy"}}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    // Second busy for an already-busy session: the `contains` guard suppresses
    // a duplicate SessionBusy.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"bb","status":{"type":"busy"}}}"#,
        "/proj",
    )
    .await;
    assert!(!drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::SessionBusy { .. })));
}

#[tokio::test]
async fn file_edited_without_path_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // No `file` property → the outer if-let fails → nothing emitted.
    handle_web_sse_event(
        &h,
        r#"{"type":"file.edited","properties":{"nope":1}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn permission_replied_unknown_request_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // requestID not present in pending_permissions → removed_sid is None → no clear.
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.replied","properties":{"requestID":"ghost"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn permission_replied_empty_request_id_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.replied","properties":{}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn question_replied_unknown_request_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"question.replied","properties":{"requestID":"ghost"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}
