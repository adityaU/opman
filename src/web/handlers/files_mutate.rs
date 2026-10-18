//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::*;
use super::super::common::resolve_project_dir;

/// POST /api/file/delete — delete a single file.
pub async fn delete_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FileDeleteRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("File not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    if !canonical_target.is_file() {
        return Err(WebError::BadRequest("Path is not a file".into()));
    }

    tokio::fs::remove_file(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to delete file: {e}")))?;

    Ok(StatusCode::OK)
}

/// POST /api/dir/delete — delete a directory recursively.
pub async fn delete_dir(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<DirDeleteRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("Directory not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    // Don't allow deleting the project root itself
    if canonical_target == canonical_base {
        return Err(WebError::BadRequest(
            "Cannot delete the project root".into(),
        ));
    }

    if !canonical_target.is_dir() {
        return Err(WebError::BadRequest("Path is not a directory".into()));
    }

    tokio::fs::remove_dir_all(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to delete directory: {e}")))?;

    Ok(StatusCode::OK)
}

/// POST /api/rename — rename or move a file/directory.
pub async fn rename_entry(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<RenameRequest>,
) -> WebResult<impl IntoResponse> {
    if req.from_path.is_empty() || req.to_path.is_empty() {
        return Err(WebError::BadRequest("Paths must not be empty".into()));
    }
    if req.from_path == req.to_path {
        return Ok(StatusCode::OK);
    }

    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;

    // Validate source exists and is within project
    let src = base.join(&req.from_path);
    let canonical_src = src
        .canonicalize()
        .map_err(|_| WebError::NotFound("Source path not found"))?;
    if !canonical_src.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }
    if canonical_src == canonical_base {
        return Err(WebError::BadRequest(
            "Cannot rename the project root".into(),
        ));
    }

    // Validate destination parent exists and is within project
    let dst = base.join(&req.to_path);
    let dst_parent = dst
        .parent()
        .ok_or(WebError::BadRequest("Invalid destination path".into()))?;
    let canonical_dst_parent = dst_parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Destination parent directory not found"))?;
    if !canonical_dst_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    // Don't overwrite existing entries
    if dst.exists() {
        return Err(WebError::BadRequest("Destination already exists".into()));
    }

    tokio::fs::rename(&canonical_src, &dst)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to rename: {e}")))?;

    // Notify editor subscribers
    let _ = state.editor_tx.send(EditorEvent::FileChanged {
        path: req.to_path.clone(),
        source: "web_rename".to_string(),
    });

    Ok(StatusCode::OK)
}
