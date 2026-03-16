//! Project management API — matches React `api/project.ts`.

use serde::Serialize;
use crate::types::api::{BrowseDirsResponse, NewSessionResponse, AppState};
use super::client::{api_fetch, api_post, api_post_void, ApiError};

// ── API functions ───────────────────────────────────────────────────

/// Switch active project by index.
pub async fn switch_project(index: usize) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body {
        index: usize,
    }
    api_post_void("/project/switch", &Body { index }).await
}

/// Add a new project by path.
pub async fn add_project(path: &str, name: Option<&str>) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        path: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<&'a str>,
    }
    api_post_void("/project/add", &Body { path, name }).await
}

/// Remove a project by index.
pub async fn remove_project(index: usize) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body {
        index: usize,
    }
    api_post_void("/project/remove", &Body { index }).await
}

/// Browse directories for project picker.
pub async fn browse_dirs(path: &str) -> Result<BrowseDirsResponse, ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        path: &'a str,
    }
    api_post("/dirs/browse", &Body { path }).await
}

/// Get user home directory.
pub async fn get_home_dir() -> Result<String, ApiError> {
    #[derive(serde::Deserialize)]
    struct Resp {
        path: String,
    }
    let resp: Resp = api_fetch("/dirs/home").await?;
    Ok(resp.path)
}

/// Select a session within a project.
pub async fn select_session(project_idx: usize, session_id: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        project_idx: usize,
        session_id: &'a str,
    }
    api_post_void("/session/select", &Body { project_idx, session_id }).await
}

/// Create a new session in a project.
pub async fn new_session(project_idx: usize) -> Result<NewSessionResponse, ApiError> {
    #[derive(Serialize)]
    struct Body {
        project_idx: usize,
    }
    api_post("/session/new", &Body { project_idx }).await
}

/// Toggle panel visibility.
pub async fn toggle_panel(panel: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        panel: &'a str,
    }
    api_post_void("/panel/toggle", &Body { panel }).await
}

/// Focus a panel.
pub async fn focus_panel(panel: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        panel: &'a str,
    }
    api_post_void("/panel/focus", &Body { panel }).await
}

/// Fetch full application state.
pub async fn fetch_app_state() -> Result<AppState, ApiError> {
    api_fetch("/state").await
}
