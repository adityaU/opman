//! Authentication and health-check handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Json};
use axum::http::HeaderMap;

use super::super::auth::{self, AuthUser, JWT_EXPIRY_SECS};
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

/// Build a `Set-Cookie` header value for the JWT auth cookie.
/// Adds the `Secure` attribute when the request arrived over HTTPS
/// (detected via the `X-Forwarded-Proto` header set by reverse proxies/tunnels).
fn jwt_cookie(token: &str, is_https: bool) -> String {
    let secure = if is_https { "; Secure" } else { "" };
    format!(
        "opman_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={JWT_EXPIRY_SECS}{secure}"
    )
}

/// Returns `true` when the request was made over HTTPS (directly or via proxy).
fn request_is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

pub async fn login(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> WebResult<impl IntoResponse> {
    let token = if state.username.is_empty() {
        auth::create_jwt("anonymous", &state.jwt_secret)?
    } else if req.username == state.username
        && constant_time_eq(req.password.as_bytes(), state.password.as_bytes())
    {
        auth::create_jwt(&req.username, &state.jwt_secret)?
    } else {
        return Err(WebError::Unauthorized);
    };

    // Return JSON body (backward compat) AND set an HttpOnly cookie
    // so the browser persists auth across tabs and page reloads.
    let is_https = request_is_https(&headers);
    Ok((
        [(SET_COOKIE, jwt_cookie(&token, is_https))],
        Json(LoginResponse { token }),
    ))
}

pub async fn verify(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let _ = state;
    Ok(StatusCode::OK)
}
