//! Editor LSP proxy handlers (diagnostics, hover, definition, format).

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_editor_buffer;

pub async fn editor_lsp_diagnostics(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_diagnostics(&socket, buf, true)
        .map_err(|e| WebError::Internal(format!("Failed to get diagnostics: {e}")))?;
    let diagnostics: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!([]));
    Ok(Json(serde_json::json!({
        "available": true,
        "diagnostics": diagnostics,
    })))
}

pub async fn editor_lsp_hover(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_hover(&socket, buf, query.line, query.col)
        .map_err(|e| WebError::Internal(format!("Failed to get hover: {e}")))?;
    let hover = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(v) if v.get("error").is_some() => None,
        _ => Some(raw),
    };
    Ok(Json(serde_json::json!({
        "available": true,
        "hover": hover,
    })))
}

pub async fn editor_lsp_definition(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_definition(&socket, buf, query.line, query.col)
        .map_err(|e| WebError::Internal(format!("Failed to get definition: {e}")))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let locations = parsed
        .get("locations")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(Json(serde_json::json!({
        "available": true,
        "locations": locations,
    })))
}

pub async fn editor_lsp_format(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<EditorFormatRequest>,
) -> WebResult<impl IntoResponse> {
    let (socket, resolved, buf) = resolve_editor_buffer(&state, &req.session_id, &req.path).await?;
    let _ = crate::nvim_rpc::nvim_lsp_format(&socket, buf)
        .map_err(|e| WebError::Internal(format!("Failed to format file: {e}")))?;
    let _ = crate::nvim_rpc::nvim_write(&socket, buf, false);
    let content = tokio::fs::read_to_string(&resolved)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read formatted file: {e}")))?;
    Ok(Json(serde_json::json!({
        "available": true,
        "formatted": true,
        "content": content,
    })))
}
