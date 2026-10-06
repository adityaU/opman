//! `ANY /remote/{id}/api/{*rest}` and the SPA routes beside it.
//!
//! Bodies stream in both directions. A small request body with a known length is
//! buffered so it can be replayed after a re-login; anything else streams straight
//! through. The response is never buffered, so SSE events reach the browser as the
//! remote emits them.

use axum::body::{Body, HttpBody};
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::{Path, Request, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};

use super::{find, gateway_error, ws_proxy, WebError};
use crate::remote::{
    downstream_response, upstream_query, upstream_request, Outbound, OutboundBody, ServerId,
};
use crate::web::auth::AuthUser;
use crate::web::static_files::serve_react_index;
use crate::web::types::ServerState;

/// Request bodies up to this size (with a declared length) are buffered and replayable.
const REPLAYABLE_LIMIT: u64 = 1024 * 1024;

pub async fn forward(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path((id, rest)): Path<(String, String)>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
    request: Request,
) -> Result<Response, WebError> {
    let remote = find(&state, &id).await?;
    let query = upstream_query(request.uri().query());
    if let Ok(upgrade) = upgrade {
        let path = match &query {
            Some(q) => format!("/api/{rest}?{q}"),
            None => format!("/api/{rest}"),
        };
        return ws_proxy::accept(upgrade, &state.remote, &remote.record.id, &path).await;
    }
    let (parts, body) = request.into_parts();
    let outbound = Outbound {
        url: remote.record.url.api(&rest, query.as_deref()),
        headers: upstream_request(&parts.headers),
        body: outbound_body(body).await?,
        method: parts.method,
    };
    let response = remote
        .send(state.remote.http(), outbound)
        .await
        .map_err(gateway_error)?;
    Ok(relay(response))
}

async fn outbound_body(body: Body) -> Result<OutboundBody, WebError> {
    match body.size_hint().exact() {
        Some(len) if len <= REPLAYABLE_LIMIT => {
            let limit = usize::try_from(len).unwrap_or(usize::MAX);
            let bytes = axum::body::to_bytes(body, limit)
                .await
                .map_err(|e| WebError::BadRequest(format!("request body: {e}")))?;
            Ok(OutboundBody::Replayable(bytes))
        }
        _ => Ok(OutboundBody::Streamed(reqwest::Body::wrap_stream(
            body.into_data_stream(),
        ))),
    }
}

/// The remote's response, status and filtered headers intact, body streamed.
fn relay(response: reqwest::Response) -> Response {
    let status = response.status();
    let headers = downstream_response(response.headers());
    let mut out = Response::new(Body::from_stream(response.bytes_stream()));
    *out.status_mut() = status;
    *out.headers_mut() = headers;
    out
}

/// `/remote/{id}` → `/remote/{id}/`, so relative URLs in the SPA resolve under the base.
pub async fn add_slash(
    auth: Result<AuthUser, WebError>,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    known(&state, auth, &id).await?;
    Ok(Redirect::temporary(&format!("/remote/{id}/")).into_response())
}

pub async fn spa_root(
    auth: Result<AuthUser, WebError>,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, WebError> {
    known(&state, auth, &id).await?;
    Ok(serve_react_index(&state, &headers).await)
}

pub async fn spa(
    auth: Result<AuthUser, WebError>,
    State(state): State<ServerState>,
    Path((id, path)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, WebError> {
    if path == "api" || path.starts_with("api/") {
        return Err(WebError::NotFound("no such API route"));
    }
    known(&state, auth, &id).await?;
    Ok(serve_react_index(&state, &headers).await)
}

/// The SPA page is the same public `index.html` that `/` serves, and it is where a
/// signed-out user logs in — after a home restart every cookie is stale, and a JSON 401
/// here would strand them. So a signed-out request gets the page without the id being
/// checked (which would also leak which ids exist); a signed-in one gets a 404 for an
/// unknown id.
async fn known(
    state: &ServerState,
    auth: Result<AuthUser, WebError>,
    id: &str,
) -> Result<(), WebError> {
    if auth.is_err() {
        return match ServerId::parse(id) {
            Some(_) => Ok(()),
            None => Err(WebError::NotFound("unknown server")),
        };
    }
    find(state, id).await.map(drop)
}
