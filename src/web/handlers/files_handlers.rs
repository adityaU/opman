//! File browsing, reading, writing, and helper utilities.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;

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
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| WebError::Internal(format!("Failed to read metadata: {e}")))?;
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

    Ok(Json(FileBrowseResponse {
        path: rel,
        entries,
    }))
}

/// GET /api/file/read?path=... — read file content.
pub async fn read_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&query.path);

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
/// Used for binary files (images, audio, video, PDFs, etc).
pub async fn read_file_raw(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&query.path);

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

    let bytes = tokio::fs::read(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read file: {e}")))?;

    let content_type = mime_from_extension(&query.path);

    Ok((
        [(axum::http::header::CONTENT_TYPE, content_type)],
        bytes,
    ))
}

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
    let parent = target.parent().ok_or(WebError::BadRequest(
        "Invalid file path".into(),
    ))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Parent directory not found"))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    tokio::fs::write(&target, &req.content)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to write file: {e}")))?;

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
    let parent = target.parent().ok_or(WebError::BadRequest(
        "Invalid file path".into(),
    ))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Parent directory not found"))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    // Don't overwrite existing files
    if target.exists() {
        return Err(WebError::BadRequest("File already exists".into()));
    }

    tokio::fs::write(&target, &req.content)
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
        return Err(WebError::BadRequest("Cannot delete the project root".into()));
    }

    if !canonical_target.is_dir() {
        return Err(WebError::BadRequest("Path is not a directory".into()));
    }

    tokio::fs::remove_dir_all(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to delete directory: {e}")))?;

    Ok(StatusCode::OK)
}

/// POST /api/file/upload — upload one or more files via multipart form data.
pub async fn upload_files(
    State(state): State<ServerState>,
    _auth: AuthUser,
    mut multipart: axum::extract::Multipart,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;

    let mut upload_dir = String::new();
    let mut saved_files = Vec::new();
    let mut pending_files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "directory" {
            // This is the directory field — read its text value
            let text = field
                .text()
                .await
                .map_err(|e| WebError::Internal(format!("Failed to read directory field: {e}")))?;
            upload_dir = text;
            continue;
        }

        // This is a file field — get the original filename
        let filename = match field.file_name() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
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

        pending_files.push((filename, data));
    }

    // Now write all files into the target directory
    for (filename, data) in pending_files {
        let rel_path = if upload_dir.is_empty() || upload_dir == "." {
            filename.clone()
        } else {
            format!("{}/{}", upload_dir, filename)
        };

        let target = base.join(&rel_path);

        // Security: validate parent is within project
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| WebError::Internal(format!("Failed to create parent dirs: {e}")))?;

            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| WebError::Internal(format!("Failed to resolve upload path: {e}")))?;
            if !canonical_parent.starts_with(&canonical_base) {
                return Err(WebError::BadRequest("Path traversal not allowed".into()));
            }
        }

        tokio::fs::write(&target, data)
            .await
            .map_err(|e| WebError::Internal(format!("Failed to write uploaded file: {e}")))?;

        saved_files.push(rel_path);
    }

    if saved_files.is_empty() {
        return Err(WebError::BadRequest("No files in upload".into()));
    }

    Ok(Json(FileUploadResponse { files: saved_files }))
}

/// Map file extension to MIME type for binary file serving.
pub(super) fn mime_from_extension(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "weba" => "audio/webm",
        // Video
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        // Documents
        "pdf" => "application/pdf",
        "csv" => "text/csv",
        "xlsx" | "xls" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" | "ppt" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "docx" | "doc" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        // Fallback
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Detect language from file extension for CodeMirror syntax highlighting.
pub(super) fn detect_language(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "py" | "pyw" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
        "json" => "json",
        "html" | "htm" => "html",
        "css" | "scss" | "less" => "css",
        "md" | "mdx" | "markdown" => "markdown",
        "sql" => "sql",
        "xml" | "svg" => "xml",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "shell",
        "fish" => "shell",
        "lua" => "lua",
        "rb" => "ruby",
        "php" => "php",
        "vue" => "vue",
        "svelte" => "svelte",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "mmd" | "mermaid" => "mermaid",
        "ini" | "cfg" | "conf" => "ini",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "diff" | "patch" => "diff",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        _ => "text",
    }
    .to_string()
}
