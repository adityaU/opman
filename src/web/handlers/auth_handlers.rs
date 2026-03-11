//! Authentication and health-check handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::{self, AuthUser};
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::constant_time_eq;

// ── Health check (unauthenticated) ──────────────────────────────────

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Auth endpoints ──────────────────────────────────────────────────

pub async fn login(
    State(state): State<ServerState>,
    Json(req): Json<LoginRequest>,
) -> WebResult<impl IntoResponse> {
    if state.username.is_empty() {
        let token = auth::create_jwt("anonymous", &state.jwt_secret)?;
        return Ok(Json(LoginResponse { token }));
    }
    if req.username == state.username && constant_time_eq(req.password.as_bytes(), state.password.as_bytes()) {
        let token = auth::create_jwt(&req.username, &state.jwt_secret)?;
        Ok(Json(LoginResponse { token }))
    } else {
        Err(WebError::Unauthorized)
    }
}

pub async fn verify(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let _ = state;
    Ok(StatusCode::OK)
}
