//! Document read/write HTTP handlers.
//! Thin layer — delegates actual format conversion to `doc_converters`.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use super::doc_readers;
use super::doc_writers;

/// GET /api/file/doc-read?path=... — read document and return structured JSON.
pub async fn doc_read(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&query.path);

    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("File not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let ext = query
        .path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    let result = tokio::task::spawn_blocking(move || match ext.as_str() {
        "xlsx" | "xls" | "ods" | "xlsb" => doc_readers::read_spreadsheet(&canonical_target),
        "docx" => doc_readers::read_docx(&canonical_target),
        "tsv" => doc_readers::read_tsv(&canonical_target),
        _ => Err(format!("Unsupported document format: .{ext}")),
    })
    .await
    .map_err(|e| WebError::Internal(format!("Task join error: {e}")))?
    .map_err(WebError::Internal)?;

    Ok(Json(DocReadResponse {
        path: query.path,
        data: result,
    }))
}

/// POST /api/file/doc-write — write structured JSON back to document format.
pub async fn doc_write(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<DocWriteRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

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

    let ext = req
        .path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let data = req.data.clone();
    let target_path = target.clone();

    tokio::task::spawn_blocking(move || match ext.as_str() {
        "xlsx" => doc_writers::write_xlsx(&target_path, &data),
        "tsv" => doc_writers::write_tsv(&target_path, &data),
        "docx" => doc_writers::write_docx(&target_path, &data),
        _ => Err(format!("Unsupported write format: .{ext}")),
    })
    .await
    .map_err(|e| WebError::Internal(format!("Task join error: {e}")))?
    .map_err(WebError::Internal)?;

    // Notify editor SSE subscribers
    let _ = state.editor_tx.send(EditorEvent::FileChanged {
        path: req.path,
        source: "web_save".to_string(),
    });

    Ok(axum::http::StatusCode::OK)
}
