//! Generated coverage tests for `health_handlers.rs`.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use axum::extract::{Query, State};
use axum::response::IntoResponse;

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

async fn json_of(resp: axum::response::Response) -> (axum::http::StatusCode, serde_json::Value) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn health_status_ok() {
    let state = test_server_state();
    let resp = get_health_status(State(state), auth())
        .await
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(v.get("config").is_some());
    assert!(v.get("snapshot").is_some());
    // there are 6 toggleable mitigations
    assert_eq!(v["mitigations"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn health_audit_with_limit() {
    let state = test_server_state();
    let resp = get_health_audit(
        State(state),
        auth(),
        Query(AuditQueryParams { limit: Some(5) }),
    )
    .await
    .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(v.get("entries").is_some());
}

#[tokio::test]
async fn health_audit_default_limit() {
    let state = test_server_state();
    let resp = get_health_audit(
        State(state),
        auth(),
        Query(AuditQueryParams { limit: None }),
    )
    .await
    .into_response();
    let (st, _) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
}

#[tokio::test]
async fn health_toggle_updates_config() {
    let state = test_server_state();
    let resp = toggle_health_mitigation(
        State(state),
        auth(),
        axum::Json(HealthToggleRequest {
            mitigation: crate::process_health::Mitigation::OrphanCleanup,
            enabled: false,
        }),
    )
    .await
    .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["config"]["orphan_cleanup"], false);
}

#[tokio::test]
async fn health_set_config_replaces() {
    let state = test_server_state();
    let cfg = crate::process_health::MitigationConfig {
        orphan_cleanup: false,
        port_cleanup: false,
        temp_cleanup: true,
        fd_watchdog: false,
        memory_watchdog: true,
        connection_watchdog: false,
    };
    let resp = set_health_config(
        State(state),
        auth(),
        axum::Json(HealthConfigRequest { config: cfg }),
    )
    .await
    .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["config"]["temp_cleanup"], true);
    assert_eq!(v["config"]["orphan_cleanup"], false);
}
