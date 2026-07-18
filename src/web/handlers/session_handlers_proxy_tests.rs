//! Generated tests (wave 2) for `session_handlers.rs`.
//!
//! These exercise the pure response-mapping helpers extracted from the
//! opencode-proxy handlers. Because there is no upstream server in tests, the
//! `.send().await` line of each handler always errors; the parsing/mapping
//! logic below is where the real behaviour lives, so we test it directly with
//! crafted upstream payloads (success, error, all body shapes).

use super::*;
use axum::http::StatusCode;
use serde_json::json;

// ── map_send_message_response ───────────────────────────────────────

#[test]
fn send_message_success_relays_body_verbatim() {
    let body = json!({ "id": "msg_1", "parts": [{ "type": "text", "text": "hi" }] });
    let out = map_send_message_response("sess_1", StatusCode::OK, body.clone());
    let Json(v) = out.expect("success should be Ok");
    assert_eq!(v, body);
}

#[test]
fn send_message_success_created_status() {
    let out = map_send_message_response("s", StatusCode::CREATED, json!({ "ok": 1 }));
    assert!(out.is_ok());
}

#[test]
fn send_message_success_null_body() {
    let out = map_send_message_response("s", StatusCode::OK, serde_json::Value::Null);
    let Json(v) = out.expect("ok");
    assert!(v.is_null());
}

#[test]
fn send_message_error_status_surfaces_internal() {
    let body = json!({ "error": "bad model" });
    let err = map_send_message_response("sess_9", StatusCode::BAD_REQUEST, body)
        .err()
        .expect("non-2xx should be Err");
    match err {
        WebError::Internal(msg) => {
            assert!(msg.contains("400"), "msg: {msg}");
            assert!(msg.contains("bad model"), "msg: {msg}");
        }
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn send_message_error_500_with_null_body() {
    let err = map_send_message_response("s", StatusCode::INTERNAL_SERVER_ERROR, serde_json::Value::Null)
        .err()
        .expect("err");
    match err {
        WebError::Internal(msg) => assert!(msg.contains("500")),
        other => panic!("expected Internal, got {other:?}"),
    }
}

// ── map_proxy_json_response (queue + rename) ────────────────────────

#[test]
fn proxy_json_success_relays_array_body() {
    let body = json!(["prompt one", "prompt two"]);
    let Json(v) = map_proxy_json_response(StatusCode::OK, body.clone()).expect("ok");
    assert_eq!(v, body);
}

#[test]
fn proxy_json_success_relays_object_body() {
    let body = json!({ "title": "renamed session" });
    let Json(v) = map_proxy_json_response(StatusCode::OK, body.clone()).expect("ok");
    assert_eq!(v["title"], "renamed session");
    assert_eq!(v, body);
}

#[test]
fn proxy_json_success_empty_object() {
    let Json(v) = map_proxy_json_response(StatusCode::OK, json!({})).expect("ok");
    assert!(v.as_object().unwrap().is_empty());
}

#[test]
fn proxy_json_error_status_is_internal() {
    let err = map_proxy_json_response(StatusCode::NOT_FOUND, json!({ "message": "no session" }))
        .err()
        .expect("err");
    match err {
        WebError::Internal(msg) => {
            assert!(msg.contains("404"), "msg: {msg}");
            assert!(msg.contains("no session"), "msg: {msg}");
        }
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn proxy_json_error_502_null_body() {
    let err = map_proxy_json_response(StatusCode::BAD_GATEWAY, serde_json::Value::Null)
        .err()
        .expect("err");
    matches!(err, WebError::Internal(_)).then_some(()).expect("internal");
}

// ── map_status_only_response (delete) ───────────────────────────────

#[test]
fn status_only_success_returns_ok_code() {
    let code = map_status_only_response(StatusCode::OK, serde_json::Value::Null).expect("ok");
    assert_eq!(code, StatusCode::OK);
}

#[test]
fn status_only_success_204_still_maps_to_200() {
    // Any 2xx upstream status collapses to a plain 200 OK for the client.
    let code = map_status_only_response(StatusCode::NO_CONTENT, serde_json::Value::Null).expect("ok");
    assert_eq!(code, StatusCode::OK);
}

#[test]
fn status_only_error_carries_upstream_details() {
    let err = map_status_only_response(StatusCode::FORBIDDEN, json!({ "message": "denied" }))
        .err()
        .expect("err");
    match err {
        WebError::Internal(msg) => {
            assert!(msg.contains("403"), "msg: {msg}");
            assert!(msg.contains("denied"), "msg: {msg}");
        }
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[test]
fn status_only_error_with_null_body() {
    let err = map_status_only_response(StatusCode::INTERNAL_SERVER_ERROR, serde_json::Value::Null)
        .err()
        .expect("err");
    matches!(err, WebError::Internal(_)).then_some(()).expect("internal");
}
