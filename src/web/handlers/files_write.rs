//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::*;
use super::super::common::resolve_project_dir;

/// POST /api/file/write — write file content.
pub async fn write_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FileWriteRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check — for writes, we can't canonicalize if file doesn't exist yet,
    // so we canonicalize the parent instead
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let parent = target
        .parent()
        .ok_or(WebError::BadRequest("Invalid file path".into()))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Parent directory not found"))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    // Write to the canonicalized path to prevent symlink race (TOCTOU)
    let canonical_target = canonical_parent.join(target.file_name().unwrap_or_default());
    tokio::fs::write(&canonical_target, &req.content)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to write file: {e}")))?;

    // Notify editor SSE subscribers about the file change.
    let _ = state.editor_tx.send(EditorEvent::FileChanged {
        path: req.path.clone(),
        source: "web_save".to_string(),
    });

    Ok(StatusCode::OK)
}

/// POST /api/file/create — create a new file.
pub async fn create_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FileCreateRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check — canonicalize the parent
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let parent = target
        .parent()
        .ok_or(WebError::BadRequest("Invalid file path".into()))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Parent directory not found"))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    // Write to the canonicalized path to prevent symlink race (TOCTOU)
    let canonical_target = canonical_parent.join(target.file_name().unwrap_or_default());

    // Don't overwrite existing files
    if canonical_target.exists() {
        return Err(WebError::BadRequest("File already exists".into()));
    }

    tokio::fs::write(&canonical_target, &req.content)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to create file: {e}")))?;

    Ok(StatusCode::CREATED)
}

/// POST /api/dir/create — create a new directory (including nested).
pub async fn create_dir(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<DirCreateRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check — we canonicalize the existing portion of the path
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;

    // Walk up to find an existing ancestor to canonicalize
    let mut check = target.as_path();
    loop {
        if check.exists() {
            let canonical = check
                .canonicalize()
                .map_err(|e| WebError::Internal(format!("Failed to resolve path: {e}")))?;
            if !canonical.starts_with(&canonical_base) {
                return Err(WebError::BadRequest("Path traversal not allowed".into()));
            }
            break;
        }
        match check.parent() {
            Some(p) => check = p,
            None => return Err(WebError::BadRequest("Invalid path".into())),
        }
    }

    if target.exists() {
        return Err(WebError::BadRequest("Directory already exists".into()));
    }

    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to create directory: {e}")))?;

    Ok(StatusCode::CREATED)
}
