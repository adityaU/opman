//! Coverage tests for the presence handlers.
use crate::web::test_support::{send_json, test_router, test_server_state};
use axum::http::StatusCode;
use serde_json::json;

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap_or(serde_json::Value::Null)
}

// ── presence ────────────────────────────────────────────────────────

#[tokio::test]
async fn presence_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/presence", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json(&body)["clients"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn presence_register_then_list_then_deregister() {
    let state = test_server_state();
    let (s1, _) = send_json(
        test_router(state.clone()),
        "POST",
        "/api/presence",
        Some(json!({ "client_id": "c1", "interface_type": "web", "focused_session": "sess" })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, body) = send_json(test_router(state.clone()), "GET", "/api/presence", None).await;
    assert_eq!(s2, StatusCode::OK);
    let clients = body_json(&body);
    assert_eq!(clients["clients"].as_array().unwrap().len(), 1);
    assert_eq!(clients["clients"][0]["client_id"], "c1");

    let (s3, _) = send_json(
        test_router(state.clone()),
        "DELETE",
        "/api/presence",
        Some(json!({ "client_id": "c1" })),
    )
    .await;
    assert_eq!(s3, StatusCode::OK);

    let (_, body2) = send_json(test_router(state), "GET", "/api/presence", None).await;
    assert_eq!(body_json(&body2)["clients"].as_array().unwrap().len(), 0);
}
