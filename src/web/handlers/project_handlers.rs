//! Project management, session selection, directory browsing, and panel handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use std::path::PathBuf;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use crate::app::base_url;

pub async fn switch_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SwitchProjectRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.switch_project(req.index).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project index".into()))
    }
}

pub async fn select_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SelectSessionRequest>,
) -> WebResult<impl IntoResponse> {
    if state
        .web_state
        .select_session(req.project_idx, req.session_id)
        .await
    {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project or session".into()))
    }
}

pub async fn new_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<NewSessionRequest>,
) -> WebResult<impl IntoResponse> {
    // Resolve the project directory for the opencode server header.
    let dir = state
        .web_state
        .get_project_working_dir(req.project_idx)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .ok_or(WebError::BadRequest("Invalid project index".into()))?;

    // Create the session synchronously via the opencode server API.
    let base = base_url().to_string();
    let resp = state
        .http_client
        .post(format!("{}/session", base))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }

    // Parse session info from the response.
    let session_info: crate::app::SessionInfo = serde_json::from_value(body.clone())
        .map_err(|e| WebError::Internal(format!("Failed to parse session info: {e}")))?;

    let session_id = session_info.id.clone();

    // Add the new session to web_state and set it as active.
    state
        .web_state
        .add_and_activate_session(req.project_idx, session_info)
        .await;

    Ok(Json(NewSessionResponse { session_id }))
}

/// POST /api/project/add — add a new project by directory path.
pub async fn add_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<AddProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.add_project(&req.path, req.name.as_deref()).await {
        Ok((index, name)) => Ok(Json(AddProjectResponse { index, name })),
        Err(msg) => Err(WebError::BadRequest(msg)),
    }
}

/// POST /api/project/remove — remove a project by index.
pub async fn remove_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<RemoveProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.remove_project(req.index).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(msg) => Err(WebError::BadRequest(msg)),
    }
}

/// GET /api/dirs/home — return the user's home directory.
pub async fn home_dir(
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .to_string_lossy()
        .to_string();
    Ok(Json(HomeDirResponse { path: home }))
}

/// Directories to skip when browsing (mirrors the TUI fuzzy picker's filter).
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "__pycache__", ".git", "vendor",
    "dist", "build", ".cache", "Library", "Pictures", "Music", "Movies",
];

/// POST /api/dirs/browse — list subdirectories of a given path.
///
/// Used by the add-project modal to let users browse the filesystem.
/// Mirrors the TUI fuzzy picker logic: skips hidden dirs and common
/// non-project directories, marks existing projects with a flag.
pub async fn browse_dirs(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<BrowseDirsRequest>,
) -> WebResult<impl IntoResponse> {
    let target = if req.path.is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    } else {
        // Expand ~ to home
        let expanded = if req.path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(req.path.trim_start_matches('~').trim_start_matches('/'))
            } else {
                PathBuf::from(&req.path)
            }
        } else {
            PathBuf::from(&req.path)
        };
        expanded
    };

    let canonical = std::fs::canonicalize(&target)
        .map_err(|e| WebError::BadRequest(format!("Invalid path: {e}")))?;

    if !canonical.is_dir() {
        return Err(WebError::BadRequest("Path is not a directory".into()));
    }

    // Collect existing project paths for marking
    let existing_paths: std::collections::HashSet<String> =
        state.web_state.all_project_paths().await.into_iter().collect();

    let canonical_str = canonical.to_string_lossy().to_string();
    let parent = canonical.parent()
        .map(|p| p.to_string_lossy().to_string())
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

        // Skip hidden directories
        if name.starts_with('.') {
            continue;
        }

        // Skip non-project directories (mirrors TUI logic)
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !metadata.is_dir() {
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
        });
    }

    // Sort alphabetically
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(Json(BrowseDirsResponse {
        path: canonical_str,
        parent,
        entries,
    }))
}

pub async fn toggle_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<TogglePanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.toggle_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}

pub async fn focus_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FocusPanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.focus_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}
