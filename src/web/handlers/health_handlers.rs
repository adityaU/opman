//! Process health API handlers.
//!
//! Provides REST endpoints for the process health bottom drawer:
//! - GET  /api/health/status — current config + snapshot + mitigation list
//! - GET  /api/health/audit  — recent audit log entries
//! - POST /api/health/toggle — enable/disable a single mitigation
//! - POST /api/health/config — replace entire mitigation config

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::health::*;
use super::super::types::ServerState;

/// GET /api/health/status
pub async fn get_health_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let handle = &state.health;
    let config = handle.config().await;
    let snapshot = handle.snapshot().await;
    Json(HealthStatusResponse::build(&config, &snapshot))
}

/// GET /api/health/audit?limit=N
pub async fn get_health_audit(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(params): axum::extract::Query<AuditQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(500);
    let entries = state.health.audit_recent(limit).await;
    Json(HealthAuditResponse { entries })
}

#[derive(serde::Deserialize)]
pub struct AuditQueryParams {
    pub limit: Option<usize>,
}

/// POST /api/health/toggle
pub async fn toggle_health_mitigation(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(body): Json<HealthToggleRequest>,
) -> impl IntoResponse {
    state.health.toggle(body.mitigation, body.enabled).await;
    let config = state.health.config().await;
    let snapshot = state.health.snapshot().await;
    Json(HealthStatusResponse::build(&config, &snapshot))
}

/// POST /api/health/config
pub async fn set_health_config(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(body): Json<HealthConfigRequest>,
) -> impl IntoResponse {
    state.health.set_config(body.config).await;
    let config = state.health.config().await;
    let snapshot = state.health.snapshot().await;
    Json(HealthStatusResponse::build(&config, &snapshot))
}
