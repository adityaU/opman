//! HTTP surface for remote servers: the `/api/servers` CRUD, the `/remote/{id}/api/*`
//! proxy (HTTP, SSE and WebSocket), and home's SPA under `/remote/{id}/`.
//!
//! The server list itself lives in [`crate::remote::Registry`]; this module only maps it
//! onto routes and responses.

mod proxy;
mod servers;
mod ws_proxy;

use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::{any, get, patch};
use axum::Router;

use super::error::WebError;
use super::types::ServerState;
use crate::remote::{Remote, RemoteError, ServerId};

/// Routes nested under `/api`.
pub(super) fn api_routes() -> Router<ServerState> {
    Router::new()
        .route("/servers", get(servers::list).post(servers::add))
        .route(
            "/servers/{id}",
            patch(servers::update).delete(servers::remove),
        )
}

/// Top-level `/remote/{id}/…` routes.
pub(super) fn remote_routes() -> Router<ServerState> {
    Router::new()
        .route("/remote/{id}", get(proxy::add_slash))
        .route("/remote/{id}/", get(proxy::spa_root))
        .route("/remote/{id}/api/{*rest}", any(proxy::forward))
        .route("/remote/{id}/{*path}", get(proxy::spa))
}

/// Resolve a path segment to a configured remote, 404 otherwise.
async fn find(state: &ServerState, raw: &str) -> Result<Arc<Remote>, WebError> {
    let id = ServerId::parse(raw).ok_or(WebError::NotFound("unknown server"))?;
    state
        .remote
        .remote(&id)
        .await
        .ok_or(WebError::NotFound("unknown server"))
}

/// For the settings page: every failure is something the user can read and fix.
fn settings_error(error: RemoteError) -> WebError {
    match error {
        RemoteError::UnknownServer => WebError::NotFound("unknown server"),
        RemoteError::Store(_) => WebError::Internal(error.to_string()),
        _ => WebError::BadRequest(error.to_string()),
    }
}

/// For the proxy: never a 401, which the frontend would read as *home* logging it out.
fn gateway_error(error: RemoteError) -> WebError {
    match error {
        RemoteError::UnknownServer => WebError::NotFound("unknown server"),
        _ => WebError::Upstream(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

#[cfg(test)]
mod fake_remote;

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod proxy_tests;

#[cfg(test)]
#[path = "servers_tests.rs"]
mod servers_tests;
