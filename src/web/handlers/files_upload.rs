//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::*;
use super::super::common::resolve_project_dir;

/// POST /api/file/upload — upload one or more files via multipart form data.
pub async fn upload_files(
    State(state): State<ServerState>,
    _auth: AuthUser,
    mut multipart: axum::extract::Multipart,
) -> WebResult<impl IntoResponse> {
    tracing::info!("[upload] upload_files handler invoked");
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;

    let mut upload_dir = String::new();
    let mut saved_files = Vec::new();
    let mut pending_files: Vec<(String, Vec<u8>)> = Vec::new();

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let field_name = field.name().unwrap_or("").to_string();

                if field_name == "directory" {
                    let text = field.text().await.map_err(|e| {
                        WebError::Internal(format!("Failed to read directory field: {e}"))
                    })?;
                    upload_dir = text;
                    tracing::info!("[upload] directory = {upload_dir:?}");
                    continue;
                }

                let filename = match field.file_name() {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        tracing::warn!("[upload] field {field_name:?} missing filename, rejecting");
                        return Err(WebError::BadRequest(
                            "Upload field must have a filename".into(),
                        ));
                    }
                };

                let data: Vec<u8> = field
                    .bytes()
                    .await
                    .map_err(|e| WebError::Internal(format!("Failed to read upload data: {e}")))?
                    .to_vec();

                tracing::info!("[upload] received file {filename:?} ({} bytes)", data.len());
                pending_files.push((filename, data));
            }
            Ok(None) => break,
            Err(e) => {
                tracing::error!("[upload] multipart next_field error: {e}");
                return Err(WebError::Internal(format!("Multipart read error: {e}")));
            }
        }
    }

    // Now write all files into the target directory
    for (filename, data) in pending_files {
        // Sanitize filename: strip path separators and traversal sequences
        let safe_name = filename
            .replace("..", "_")
            .replace('/', "_")
            .replace('\\', "_");
        if safe_name.is_empty() || safe_name.starts_with('.') {
            tracing::warn!("[upload] rejected unsafe filename: {filename:?}");
            return Err(WebError::BadRequest("Invalid filename".into()));
        }

        let rel_path = if upload_dir.is_empty() || upload_dir == "." {
            safe_name.clone()
        } else {
            let safe_dir = upload_dir
                .replace("..", "_")
                .replace('/', "_")
                .replace('\\', "_");
            format!("{}/{}", safe_dir, safe_name)
        };

        let target = base.join(&rel_path);
        tracing::info!("[upload] writing {rel_path:?} → {}", target.display());

        // Security: validate the full resolved path is within project BEFORE any I/O
        let canonical_target = target
            .canonicalize()
            .or_else(|_| {
                // File doesn't exist yet — canonicalize the parent instead
                target
                    .parent()
                    .map(|p| p.canonicalize())
                    .unwrap_or(Ok(target.clone()))
            })
            .map_err(|_| WebError::BadRequest("Invalid upload path".into()))?;
        if !canonical_target.starts_with(&canonical_base) {
            return Err(WebError::BadRequest("Path traversal not allowed".into()));
        }

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| WebError::Internal(format!("Failed to create parent dirs: {e}")))?;
        }

        tokio::fs::write(&target, data)
            .await
            .map_err(|e| WebError::Internal(format!("Failed to write uploaded file: {e}")))?;

        saved_files.push(rel_path);
    }

    if saved_files.is_empty() {
        tracing::warn!("[upload] no files saved — returning error");
        return Err(WebError::BadRequest("No files in upload".into()));
    }

    tracing::info!(
        "[upload] success — saved {} files: {:?}",
        saved_files.len(),
        saved_files
    );
    Ok(Json(FileUploadResponse { files: saved_files }))
}
