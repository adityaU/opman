//! Authenticated, allowlisted adapter to the Settings handlers.
use axum::{
    body::{to_bytes, Body},
    extract::{FromRequestParts, State},
    http::{request::Parts, Request},
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use tower::ServiceExt;

use super::super::{
    auth::create_jwt,
    error::{WebError, WebResult},
    types::{ServerState, WebEvent},
};
use crate::mcp_management::catalog;

#[path = "management_output.rs"]
mod output;

pub(super) struct ManagementAuth;

impl FromRequestParts<ServerState> for ManagementAuth {
    type Rejection = WebError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let candidate = parts
            .headers
            .get("x-internal-token")
            .and_then(|value| value.to_str().ok())
            .ok_or(WebError::Unauthorized)?;
        if state.internal_token.is_empty() || candidate.is_empty() {
            return Err(WebError::Unauthorized);
        }
        // HMAC verification compares the two capabilities in constant time.
        let mut expected = Hmac::<Sha256>::new_from_slice(state.internal_token.as_bytes())
            .map_err(|_| WebError::Unauthorized)?;
        expected.update(b"opman-management");
        let mut actual = Hmac::<Sha256>::new_from_slice(candidate.as_bytes())
            .map_err(|_| WebError::Unauthorized)?;
        actual.update(b"opman-management");
        actual
            .verify_slice(&expected.finalize().into_bytes())
            .map_err(|_| WebError::Unauthorized)?;
        Ok(Self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Call {
    operation: String,
    #[serde(default)]
    args: Value,
}

pub(super) async fn call(
    _auth: ManagementAuth,
    State(state): State<ServerState>,
    Json(call): Json<Call>,
) -> WebResult<Json<Value>> {
    let operations = catalog::operations();
    if call.operation == "catalog" {
        return Ok(Json(Value::Array(
            operations
                .iter()
                .map(catalog::Operation::definition)
                .collect(),
        )));
    }
    let operation = operations
        .iter()
        .find(|operation| operation.name == call.operation)
        .ok_or_else(|| WebError::BadRequest("Unknown management operation".into()))?;
    let args = call
        .args
        .as_object()
        .ok_or_else(|| WebError::BadRequest("arguments must be an object".into()))?;
    if args.keys().any(|key| key != "id" && key != "body") {
        return Err(WebError::BadRequest("Unknown management argument".into()));
    }
    let target = operation.target(&call.args).map_err(WebError::BadRequest)?;
    let body = match &operation.body {
        Some(_) => args
            .get("body")
            .filter(|value| value.is_object())
            .ok_or_else(|| WebError::BadRequest("body must be an object".into()))?,
        None => &Value::Null,
    };
    // Only this process mints the JWT. It never leaves the in-process request.
    let jwt = create_jwt("opman-management", &state.jwt_secret)?;
    let request = Request::builder()
        .method(operation.method)
        .uri(&target)
        .header("authorization", format!("Bearer {jwt}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .map_err(|error| WebError::Internal(error.to_string()))?;
    let router = super::api::api_routes()
        .merge(super::super::remote::api_routes())
        .merge(super::super::remote::remote_routes())
        .with_state(state.clone());
    let response = router
        .oneshot(request)
        .await
        .map_err(|error| WebError::Internal(error.to_string()))?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .map_err(|_| WebError::Internal("Settings response is too large".into()))?;
    if !status.is_success() {
        // Do not echo handler errors: an upstream URL may carry a credential.
        return Err(WebError::BadRequest(format!(
            "Settings operation failed ({status})"
        )));
    }
    let mut value: Value = if bytes.is_empty() {
        json!({"status":"saved"})
    } else {
        serde_json::from_slice(&bytes)
            .map_err(|_| WebError::Internal("Invalid settings response".into()))?
    };
    if operation.name == "settings_projects_list" {
        value = json!({"projects": value.get("projects").and_then(Value::as_array).map(|projects| projects.iter().map(|project| json!({"index":project["index"],"name":project["name"],"path":project["path"]})).collect::<Vec<_>>()).unwrap_or_default()});
    }
    if operation.method != "GET" {
        let _ = state.event_tx.send(WebEvent::SettingsChanged {
            section: output::section(operation.name).into(),
        });
        let _ = state.event_tx.send(WebEvent::StateChanged);
    }
    output::redact(&mut value);
    Ok(Json(value))
}

#[cfg(test)]
#[path = "management_tests.rs"]
mod tests;
