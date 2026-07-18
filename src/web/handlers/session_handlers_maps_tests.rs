//! Generated tests (wave 2, part 3) for `session_handlers.rs`.
//!
//! Direct unit tests for the remaining pure response-mapping helpers that were
//! only exercised indirectly (through the unreachable-upstream handler tests):
//! `map_command_error`, `a2ui_callback_text`, and `a2ui_callback_result`.
//! Covers the branches the handler-level tests cannot reach (e.g. the
//! `CommandError` downcast, and the A2UI success mapping).

use super::*;
use axum::http::StatusCode;
use serde_json::json;

// ── map_command_error ───────────────────────────────────────────────

#[test]
fn command_error_downcast_maps_to_upstream_with_status() {
    let e = anyhow::Error::new(CommandError {
        status: 404,
        message: "no such command".into(),
    });
    match map_command_error(&e) {
        WebError::Upstream(status, msg) => {
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(msg, "no such command");
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

#[test]
fn command_error_downcast_preserves_400() {
    let e = anyhow::Error::new(CommandError {
        status: 400,
        message: "bad args".into(),
    });
    match map_command_error(&e) {
        WebError::Upstream(status, msg) => {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(msg, "bad args");
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

#[test]
fn command_error_invalid_status_code_falls_back_to_500() {
    // status 0 is not a valid HTTP status → StatusCode::from_u16 errors →
    // the handler substitutes INTERNAL_SERVER_ERROR but keeps the message.
    let e = anyhow::Error::new(CommandError {
        status: 0,
        message: "weird".into(),
    });
    match map_command_error(&e) {
        WebError::Upstream(status, msg) => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(msg, "weird");
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

#[test]
fn command_error_non_command_error_is_generic_internal() {
    // A plain anyhow error (not a CommandError) → the else branch.
    let e = anyhow::anyhow!("connection refused");
    match map_command_error(&e) {
        WebError::Internal(msg) => assert_eq!(msg, "Command execution failed"),
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn command_error_wrapped_context_still_downcasts() {
    // anyhow context wrapping still allows downcast_ref to find CommandError.
    let e = anyhow::Error::new(CommandError {
        status: 503,
        message: "busy".into(),
    })
    .context("while executing command");
    match map_command_error(&e) {
        WebError::Upstream(status, msg) => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(msg, "busy");
        }
        other => panic!("expected Upstream, got {other:?}"),
    }
}

// ── a2ui_callback_text ──────────────────────────────────────────────

#[test]
fn a2ui_text_null_payload_is_bare_marker() {
    let t = a2ui_callback_text("cb-42", &serde_json::Value::Null);
    assert_eq!(t, "[A2UI callback: cb-42]");
    assert!(!t.contains("```"));
}

#[test]
fn a2ui_text_empty_object_is_bare_marker() {
    let t = a2ui_callback_text("cb-empty", &json!({}));
    assert_eq!(t, "[A2UI callback: cb-empty]");
}

#[test]
fn a2ui_text_nonempty_payload_appends_fenced_json() {
    let t = a2ui_callback_text("btn1", &json!({ "field": "value", "n": 3 }));
    assert!(t.starts_with("[A2UI callback: btn1]\n```json\n"));
    assert!(t.ends_with("\n```"));
    assert!(t.contains("\"field\": \"value\""));
    assert!(t.contains("\"n\": 3"));
}

#[test]
fn a2ui_text_array_payload_is_fenced() {
    // A non-object, non-null payload (array) still goes through the fenced branch.
    let t = a2ui_callback_text("arr", &json!(["a", "b"]));
    assert!(t.contains("```json"));
    assert!(t.contains("\"a\""));
}

#[test]
fn a2ui_text_scalar_payload_is_fenced() {
    // A bare non-null scalar is neither null nor `{}` → fenced branch.
    let t = a2ui_callback_text("s", &json!(7));
    assert!(t.contains("```json"));
    assert!(t.contains('7'));
}

// ── a2ui_callback_result ────────────────────────────────────────────

#[test]
fn a2ui_result_success_returns_ok_true() {
    let Json(v) = a2ui_callback_result(StatusCode::OK, serde_json::Value::Null).expect("ok");
    assert_eq!(v, json!({ "ok": true }));
}

#[test]
fn a2ui_result_success_ignores_body_returns_ok_true() {
    // On success the upstream body is discarded; only { ok: true } is returned.
    let Json(v) =
        a2ui_callback_result(StatusCode::CREATED, json!({ "irrelevant": 1 })).expect("ok");
    assert_eq!(v, json!({ "ok": true }));
}

#[test]
fn a2ui_result_error_carries_upstream_details() {
    let err = a2ui_callback_result(StatusCode::BAD_GATEWAY, json!({ "message": "down" }))
        .err()
        .expect("err");
    match err {
        WebError::Internal(msg) => {
            assert!(msg.contains("502"), "msg: {msg}");
            assert!(msg.contains("down"), "msg: {msg}");
        }
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn a2ui_result_error_null_body() {
    let err = a2ui_callback_result(StatusCode::INTERNAL_SERVER_ERROR, serde_json::Value::Null)
        .err()
        .expect("err");
    matches!(err, WebError::Internal(_))
        .then_some(())
        .expect("internal");
}
