//! Generated coverage tests (wave 2) for `context_handlers.rs`.
//!
//! Exercises the pure `max_context_from_providers` provider-JSON walk across
//! every fallback branch (the `{ "all": [...] }` shape, the bare flat-array
//! shape, model-missing, limit-missing, and the non-array default), plus the
//! `get_context_window` path that resolves the *active* session when no
//! `session_id` query param is supplied.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use serde_json::json;

fn init_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: String::new(),
        directory: dir.into(),
        time: crate::app::SessionTime { created: 1, updated: 2 },
    }
}

// ── max_context_from_providers: "all" shape ─────────────────────────

#[test]
fn max_ctx_all_shape_picks_max() {
    let v = json!({
        "all": [
            { "models": {
                "a": { "limit": { "context": 100_000 } },
                "b": { "limit": { "context": 400_000 } }
            } },
            { "models": {
                "c": { "limit": { "context": 250_000 } }
            } }
        ]
    });
    assert_eq!(max_context_from_providers(&v), 400_000);
}

#[test]
fn max_ctx_all_shape_provider_without_models_skipped() {
    let v = json!({
        "all": [
            { "name": "no-models-here" },
            { "models": { "a": { "limit": { "context": 123_456 } } } }
        ]
    });
    assert_eq!(max_context_from_providers(&v), 123_456);
}

#[test]
fn max_ctx_all_shape_model_without_limit_falls_back() {
    // Model present but no /limit/context anywhere → max stays 0 → 200_000.
    let v = json!({
        "all": [ { "models": { "a": { "name": "x" }, "b": { "limit": {} } } } ]
    });
    assert_eq!(max_context_from_providers(&v), 200_000);
}

#[test]
fn max_ctx_all_not_an_array_falls_through() {
    // "all" present but not an array → the `.as_array()` guard fails; the flat
    // branch also finds nothing → default.
    let v = json!({ "all": { "not": "an array" } });
    assert_eq!(max_context_from_providers(&v), 200_000);
}

// ── flat top-level array shape ──────────────────────────────────────

#[test]
fn max_ctx_flat_array_shape() {
    let v = json!([
        { "models": { "a": { "limit": { "context": 32_000 } } } },
        { "models": { "b": { "limit": { "context": 128_000 } } } }
    ]);
    assert_eq!(max_context_from_providers(&v), 128_000);
}

#[test]
fn max_ctx_flat_array_provider_without_models_skipped() {
    let v = json!([
        { "id": "bare" },
        { "models": { "a": { "limit": { "context": 64_000 } } } }
    ]);
    assert_eq!(max_context_from_providers(&v), 64_000);
}

#[test]
fn max_ctx_flat_array_limit_missing_falls_back() {
    let v = json!([ { "models": { "a": { "name": "no-limit" } } } ]);
    assert_eq!(max_context_from_providers(&v), 200_000);
}

// ── neither shape / empty ───────────────────────────────────────────

#[test]
fn max_ctx_empty_object_falls_back() {
    assert_eq!(max_context_from_providers(&json!({})), 200_000);
}

#[test]
fn max_ctx_empty_array_falls_back() {
    assert_eq!(max_context_from_providers(&json!([])), 200_000);
}

#[test]
fn max_ctx_non_object_non_array_falls_back() {
    assert_eq!(max_context_from_providers(&json!("string")), 200_000);
    assert_eq!(max_context_from_providers(&json!(42)), 200_000);
    assert_eq!(max_context_from_providers(&json!(null)), 200_000);
}

#[test]
fn max_ctx_context_zero_is_not_selected() {
    // A literal context of 0 must not be treated as "found" (0 > 0 is false).
    let v = json!({ "all": [ { "models": { "a": { "limit": { "context": 0 } } } } ] });
    assert_eq!(max_context_from_providers(&v), 200_000);
}

// ── get_context_window resolving the ACTIVE session (no query param) ─

#[tokio::test]
async fn context_window_uses_active_session_when_no_query() {
    init_base_url();
    let tmp = tempfile::tempdir().unwrap();
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), None)
        .await
        .expect("add project");
    // Set an active session so the `None => active_session_id()` arm returns Some.
    state
        .web_state
        .add_and_activate_session(0, sess("live", tmp.path().to_str().unwrap()))
        .await;

    // No session_id query param → resolves the active session, then the
    // provider fetch to the dead base_url fails → 200_000 fallback.
    let (status, body) =
        send_json(test_router(state), "GET", "/api/context-window", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["context_limit"], 200_000);
    assert_eq!(v["total_used"], 0);
}
