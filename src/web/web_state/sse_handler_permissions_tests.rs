//! Generated tests for `handle_web_sse_event` (part 2): file edits, permission
//! and question tracking, session errors, and input recalculation.

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
async fn file_edited_with_active_session_emits_state_change() {
    let h = handle_for("/proj");
    // Auto-activates root session for the project.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"fe","parentID":"","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let mut rx = h.subscribe_events();
    // Absolute, nonexistent file -> record_file_edit returns early, but the
    // handler still pushes an activity event and a StateChanged.
    handle_web_sse_event(
        &h,
        r#"{"type":"file.edited","properties":{"file":"/proj/x.rs"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn file_edited_without_active_session_no_state_change() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // No active session for /proj -> only the (no-op) editor emit; no StateChanged.
    handle_web_sse_event(
        &h,
        r#"{"type":"file.edited","properties":{"file":"/proj/y.rs"}}"#,
        "/proj",
    )
    .await;
    assert!(!drain(&mut rx).iter().any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn permission_asked_then_replied() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.asked","properties":{"id":"req1","sessionID":"s1"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputNeeded { session_id } if session_id == "s1")));

    handle_web_sse_event(
        &h,
        r#"{"type":"permission.replied","properties":{"requestID":"req1"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputCleared { session_id } if session_id == "s1")));
}

#[tokio::test]
async fn permission_asked_empty_id_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.asked","properties":{"sessionID":"s1"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn question_asked_replied_and_rejected() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"question.asked","properties":{"id":"q1","sessionID":"qs"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputNeeded { .. })));

    handle_web_sse_event(
        &h,
        r#"{"type":"question.replied","properties":{"requestID":"q1"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputCleared { .. })));

    // question.rejected path (re-ask then reject).
    handle_web_sse_event(
        &h,
        r#"{"type":"question.asked","properties":{"id":"q2","sessionID":"qs"}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    handle_web_sse_event(
        &h,
        r#"{"type":"question.rejected","properties":{"id":"q2"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputCleared { .. })));
}

#[tokio::test]
async fn recalc_keeps_input_when_other_pending_remains() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // Two pending items for the same session.
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.asked","properties":{"id":"p1","sessionID":"multi"}}"#,
        "/proj",
    )
    .await;
    handle_web_sse_event(
        &h,
        r#"{"type":"question.asked","properties":{"id":"p2","sessionID":"multi"}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    // Resolve one; the other still references the session -> no clear.
    handle_web_sse_event(
        &h,
        r#"{"type":"permission.replied","properties":{"requestID":"p1"}}"#,
        "/proj",
    )
    .await;
    assert!(!drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionInputCleared { .. })));
}

#[tokio::test]
async fn session_error_root_not_active_emits_error_and_unseen() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.error","properties":{"sessionID":"e1","error":"kaboom"}}"#,
        "/proj",
    )
    .await;
    let evs = drain(&mut rx);
    assert!(evs.iter().any(|e| matches!(e, WebEvent::SessionError { session_id, message } if session_id == "e1" && message == "kaboom")));
    assert!(evs.iter().any(|e| matches!(e, WebEvent::SessionUnseen { .. })));
}

#[tokio::test]
async fn session_error_message_fallback_field() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // Uses "message" when "error" is absent.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.error","properties":{"sessionID":"e2","message":"oops"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionError { message, .. } if message == "oops")));
}

#[tokio::test]
async fn session_error_empty_session_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.error","properties":{"sessionID":""}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn session_error_subagent_skips_unseen() {
    let h = handle_for("/proj");
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"suberr","parentID":"root","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.error","properties":{"sessionID":"suberr","error":"x"}}"#,
        "/proj",
    )
    .await;
    let evs = drain(&mut rx);
    assert!(evs.iter().any(|e| matches!(e, WebEvent::SessionError { .. })));
    assert!(!evs.iter().any(|e| matches!(e, WebEvent::SessionUnseen { .. })));
}
