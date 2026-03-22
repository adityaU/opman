//! File and directory download handlers.

use axum::extract::State;
use axum::response::IntoResponse;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;

/// GET /api/file/download?path=... — download a single file as attachment.
pub async fn download_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileDownloadQuery>,
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
    if !canonical_target.is_file() {
        return Err(WebError::BadRequest("Path is not a file".into()));
    }

    let bytes = tokio::fs::read(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read file: {e}")))?;

    let filename = canonical_target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".into());

    let content_type = super::files_handlers::mime_from_extension(&query.path);
    let disposition = format!("attachment; filename=\"{}\"", filename.replace('"', "_"));

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (
                axum::http::header::CONTENT_DISPOSITION,
                disposition,
            ),
        ],
        bytes,
    ))
}

/// GET /api/dir/download?path=... — download a directory as a zip archive.
pub async fn download_dir(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<DirDownloadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let rel = if query.path.is_empty() || query.path == "." {
        ".".to_string()
    } else {
        query.path.clone()
    };
    let target = base.join(&rel);

    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("Directory not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }
    if !canonical_target.is_dir() {
        return Err(WebError::BadRequest("Path is not a directory".into()));
    }

    let zip_bytes = build_zip(&canonical_target).await.map_err(|e| {
        WebError::Internal(format!("Failed to create zip: {e}"))
    })?;

    let dir_name = canonical_target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".into());
    let filename = format!("{dir_name}.zip");
    let disposition = format!("attachment; filename=\"{}\"", filename.replace('"', "_"));

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/zip".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                disposition,
            ),
        ],
        zip_bytes,
    ))
}

/// Recursively zip a directory into an in-memory buffer.
async fn build_zip(root: &std::path::Path) -> Result<Vec<u8>, std::io::Error> {
    let root = root.to_path_buf();
    // Do zip building on a blocking thread since zip crate is synchronous
    tokio::task::spawn_blocking(move || {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        add_dir_recursive(&root, &root, &mut zip, options)?;

        let buf = zip.finish()?;
        Ok(buf.into_inner())
    })
    .await
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}

/// Walk `dir` recursively, adding entries relative to `root`.
fn add_dir_recursive(
    dir: &std::path::Path,
    root: &std::path::Path,
    zip: &mut zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
    options: zip::write::SimpleFileOptions,
) -> std::io::Result<()> {
    use std::io::Write;

    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_dir() {
            zip.add_directory(&format!("{rel}/"), options)?;
            add_dir_recursive(&path, root, zip, options)?;
        } else {
            zip.start_file(&rel, options)?;
            let data = std::fs::read(&path)?;
            zip.write_all(&data)?;
        }
    }
    Ok(())
}
