//! Editor LSP endpoints (diagnostics, hover, definition, format).
//!
//! These used to proxy to a Neovim instance, which meant they only worked when
//! the user happened to have a Neovim session open for the same project and
//! session id — so in the web editor they effectively never worked at all. They
//! now go to [`crate::lsp`], which starts the right language server for the
//! file's type in the file's own project root, on demand.
//!
//! Every handler resolves the path through the project sandbox and then hands
//! off; the LSP layer answers `available: false` rather than erroring for the
//! ordinary cases (no server for this file type, server not installed, still
//! starting), because the editor renders availability and a 500 would surface
//! as a failure for something that is merely not applicable.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_readable_path;

/// Resolve the requested path inside the project and return both it and the
/// project root the language server should be anchored under.
async fn editor_target(
    state: &ServerState,
    path: &str,
) -> WebResult<(std::path::PathBuf, std::path::PathBuf)> {
    let project_dir = state
        .web_state
        .get_working_dir()
        .await
        .ok_or_else(|| WebError::BadRequest("No active project directory".into()))?;
    let file = resolve_readable_path(&project_dir, path)?;
    Ok((file, project_dir))
}

pub async fn editor_lsp_diagnostics(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(query): Json<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (file, project_dir) = editor_target(&state, &query.path).await?;
    Ok(Json(
        crate::lsp::api::diagnostics(&state.lsp, &file, &project_dir, query.content.as_deref())
            .await,
    ))
}

pub async fn editor_lsp_hover(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(query): Json<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (file, project_dir) = editor_target(&state, &query.path).await?;
    Ok(Json(
        crate::lsp::api::hover(
            &state.lsp,
            &file,
            &project_dir,
            query.line.unwrap_or(1),
            query.col.unwrap_or(1),
            query.content.as_deref(),
        )
        .await,
    ))
}

pub async fn editor_lsp_definition(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(query): Json<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (file, project_dir) = editor_target(&state, &query.path).await?;
    Ok(Json(
        crate::lsp::api_edit::definition(
            &state.lsp,
            &file,
            &project_dir,
            query.line.unwrap_or(1),
            query.col.unwrap_or(1),
            query.content.as_deref(),
        )
        .await,
    ))
}

pub async fn editor_lsp_format(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<EditorFormatRequest>,
) -> WebResult<impl IntoResponse> {
    let (file, project_dir) = editor_target(&state, &req.path).await?;
    Ok(Json(
        crate::lsp::api_edit::format(&state.lsp, &file, &project_dir, req.content.as_deref()).await,
    ))
}

pub async fn editor_lsp_completion(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(query): Json<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (file, project_dir) = editor_target(&state, &query.path).await?;
    Ok(Json(
        crate::lsp::api::completion(
            &state.lsp,
            &file,
            &project_dir,
            query.line.unwrap_or(1),
            query.col.unwrap_or(1),
            query.content.as_deref(),
            query.trigger.as_deref(),
        )
        .await,
    ))
}

#[cfg(test)]
#[path = "editor_handlers_tests.rs"]
mod editor_handlers_tests;
