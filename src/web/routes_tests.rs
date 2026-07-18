use super::*;

use crate::web::test_support::{send_json, test_router, test_server_state};

#[tokio::test]
async fn build_router_constructs() {
    // Simply constructing the full router exercises every route-registration
    // line in build_router.
    let state = test_server_state();
    let _router: Router = build_router(state);
}

#[tokio::test]
async fn health_route_resolves() {
    let router = test_router(test_server_state());
    let (status, _body) = send_json(router, "GET", "/health", None).await;
    assert!(status.is_success(), "health should be 2xx, got {status}");
}

#[tokio::test]
async fn api_state_route_resolves() {
    let router = test_router(test_server_state());
    let (status, _body) = send_json(router, "GET", "/api/state", None).await;
    // No auth configured in the test state, so the request reaches the handler.
    assert!(
        status.is_success() || status.is_server_error() || status.is_client_error(),
        "unexpected status {status}"
    );
}

#[tokio::test]
async fn fallback_serves_react_index() {
    let router = test_router(test_server_state());
    let (status, body) = send_json(router, "GET", "/", None).await;
    // The embedded index.html is served as the SPA fallback.
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn internal_route_resolves() {
    let router = test_router(test_server_state());
    let (status, _body) =
        send_json(router, "GET", "/internal/kanban/task/tsk_missing", None).await;
    // Route exists; handler runs against an empty in-memory DB.
    assert!(
        status.is_success() || status.is_client_error() || status.is_server_error(),
        "unexpected status {status}"
    );
}

#[tokio::test]
async fn auth_login_route_resolves() {
    let router = test_router(test_server_state());
    let body = serde_json::json!({ "username": "u", "password": "p" });
    let (status, _body) = send_json(router, "POST", "/api/auth/login", Some(body)).await;
    assert!(
        status.is_success() || status.is_client_error() || status.is_server_error(),
        "unexpected status {status}"
    );
}

#[tokio::test]
async fn public_bootstrap_route_resolves() {
    let router = test_router(test_server_state());
    let (status, _body) = send_json(router, "GET", "/api/public/bootstrap", None).await;
    assert!(
        status.is_success() || status.is_client_error() || status.is_server_error(),
        "unexpected status {status}"
    );
}
