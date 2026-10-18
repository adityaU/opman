//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::*;
use super::super::common::{resolve_project_dir, resolve_readable_path};
use super::{detect_language, mime_from_extension};

/// GET /api/files?path=... — list directory contents.
pub async fn browse_files(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileBrowseQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let rel = if query.path.is_empty() {
        ".".to_string()
    } else {
        query.path.clone()
    };
    let target = base.join(&rel);

    // Security: ensure resolved path is within project dir
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("Directory not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let mut entries = Vec::new();
    let mut dir_reader = tokio::fs::read_dir(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read directory: {e}")))?;

    while let Some(entry) = dir_reader
        .next_entry()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read entry: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs (starting with .)
        if name.starts_with('.') {
            continue;
        }
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| WebError::Internal(format!("Failed to read file type: {e}")))?;
        let metadata = if file_type.is_symlink() {
            // Security: skip symlinks that escape the project directory.
            let link_target = match tokio::fs::read_link(entry.path()).await {
                Ok(t) => t,
                Err(_) => continue,
            };
            let resolved = if link_target.is_absolute() {
                link_target
            } else {
                canonical_target.join(link_target)
            };
            let canonical_link = match resolved.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };
            if !canonical_link.starts_with(&canonical_base) {
                tracing::warn!(
                    "[browse] skipping symlink escaping project: {}",
                    entry.path().display()
                );
                continue;
            }
            match tokio::fs::metadata(canonical_link).await {
                Ok(metadata) => metadata,
                Err(_) => continue,
            }
        } else {
            entry
                .metadata()
                .await
                .map_err(|e| WebError::Internal(format!("Failed to read metadata: {e}")))?
        };

        let entry_path = if rel == "." {
            name.clone()
        } else {
            format!("{}/{}", rel, name)
        };
        entries.push(FileEntry {
            name,
            path: entry_path,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(FileBrowseResponse { path: rel, entries }))
}

/// GET /api/file/read?path=... — read file content.
pub async fn read_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let canonical_target = resolve_readable_path(base, &query.path)?;

    let content = tokio::fs::read_to_string(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read file: {e}")))?;

    let language = detect_language(&query.path);

    Ok(Json(FileReadResponse {
        path: query.path,
        content,
        language,
    }))
}

/// GET /api/file/raw?path=... — serve raw file bytes with Content-Type.
/// Used for binary files (images, audio, video, PDFs, etc). Supports `Range`.
pub async fn read_file_raw(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
    headers: axum::http::HeaderMap,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let canonical_target = resolve_readable_path(base, &query.path)?;
    let content_type = mime_from_extension(&query.path);
    super::super::raw_range::serve_file(&canonical_target, content_type, &headers).await
}
