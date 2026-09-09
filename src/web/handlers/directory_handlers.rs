//! Handlers for the add-project directory browser.

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use std::path::PathBuf;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;

/// GET /api/dirs/home — return the user's home directory.
pub async fn home_dir(_auth: AuthUser) -> WebResult<impl IntoResponse> {
    let home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .to_string_lossy()
        .to_string();
    Ok(Json(HomeDirResponse { path: home }))
}

/// Directories to skip when browsing (mirrors the TUI fuzzy picker's filter).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    ".git",
    "vendor",
    "dist",
    "build",
    ".cache",
    "Library",
    "Pictures",
    "Music",
    "Movies",
];

/// POST /api/dirs/browse — list subdirectories of a given path.
pub async fn browse_dirs(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<BrowseDirsRequest>,
) -> WebResult<impl IntoResponse> {
    let target = if req.path.is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    } else if req.path.starts_with('~') {
        dirs::home_dir()
            .map(|home| home.join(req.path.trim_start_matches('~').trim_start_matches('/')))
            .unwrap_or_else(|| PathBuf::from(&req.path))
    } else {
        PathBuf::from(&req.path)
    };
    let canonical = std::fs::canonicalize(&target)
        .map_err(|e| WebError::BadRequest(format!("Invalid path: {e}")))?;
    if !canonical.is_dir() {
        return Err(WebError::BadRequest("Path is not a directory".into()));
    }

    let existing_paths: std::collections::HashSet<String> = state
        .web_state
        .all_project_paths()
        .await
        .into_iter()
        .collect();
    let canonical_str = canonical.to_string_lossy().to_string();
    let parent = canonical
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut entries = Vec::new();
    let mut dir_reader = tokio::fs::read_dir(&canonical)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read directory: {e}")))?;
    while let Some(entry) = dir_reader
        .next_entry()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read entry: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let file_type = match entry.file_type().await {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        // DirEntry metadata describes the link itself. Follow only symlinks so ordinary
        // directories keep the cheap readdir-backed classification.
        let (is_dir, is_symlink) = if file_type.is_dir() {
            (true, false)
        } else if file_type.is_symlink() {
            let is_dir = match tokio::fs::metadata(entry.path()).await {
                Ok(metadata) => metadata.is_dir(),
                Err(_) => false,
            };
            (is_dir, true)
        } else {
            (false, false)
        };
        if !is_dir {
            continue;
        }
        let entry_path = if canonical_str.ends_with('/') {
            format!("{}{}", canonical_str, name)
        } else {
            format!("{}/{}", canonical_str, name)
        };
        let is_project = existing_paths.contains(&entry_path);
        entries.push(DirEntry {
            name,
            path: entry_path,
            is_project,
            is_symlink,
        });
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(Json(BrowseDirsResponse {
        path: canonical_str,
        parent,
        entries,
    }))
}

/// POST /api/dirs/create — create one directory inside the browsed path.
pub async fn create_project_dir(
    _auth: AuthUser,
    Json(req): Json<CreateDirRequest>,
) -> WebResult<impl IntoResponse> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(WebError::BadRequest("Folder name cannot be empty".into()));
    }
    if name == "." || name == ".." {
        return Err(WebError::BadRequest("Folder name is invalid".into()));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(WebError::BadRequest(
            "Folder name cannot contain path separators".into(),
        ));
    }
    let parent = std::fs::canonicalize(&req.parent)
        .map_err(|e| WebError::BadRequest(format!("Invalid parent directory: {e}")))?;
    if !parent.is_dir() {
        return Err(WebError::BadRequest(
            "Parent path is not a directory".into(),
        ));
    }
    let path = parent.join(name);
    if path.exists() {
        return Err(WebError::BadRequest(
            "A folder with that name already exists".into(),
        ));
    }
    tokio::fs::create_dir(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            WebError::BadRequest("A folder with that name already exists".into())
        } else {
            WebError::BadRequest(format!("Failed to create folder: {e}"))
        }
    })?;
    Ok(Json(CreateDirResponse {
        name: name.to_string(),
        path: path.to_string_lossy().to_string(),
    }))
}

#[cfg(test)]
#[path = "directory_handlers_symlink_tests.rs"]
mod directory_handlers_symlink_tests;
