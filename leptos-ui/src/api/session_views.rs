//! Session views API — matches React `api/session-views.ts`.

use crate::types::api::{
    SessionsOverviewResponse, SessionsTreeResponse, ContextWindowResponse,
    FileEditsResponse, SearchResponse,
};
use super::client::{api_fetch, ApiError};

// ── API functions ───────────────────────────────────────────────────

/// Fetch multi-session dashboard overview.
pub async fn fetch_sessions_overview() -> Result<SessionsOverviewResponse, ApiError> {
    api_fetch("/sessions/overview").await
}

/// Fetch session tree view.
pub async fn fetch_sessions_tree() -> Result<SessionsTreeResponse, ApiError> {
    api_fetch("/sessions/tree").await
}

/// Fetch context window usage for a session.
pub async fn fetch_context_window(session_id: Option<&str>) -> Result<ContextWindowResponse, ApiError> {
    let path = match session_id {
        Some(sid) => format!(
            "/context-window?session_id={}",
            js_sys::encode_uri_component(sid),
        ),
        None => "/context-window".to_string(),
    };
    api_fetch(&path).await
}

/// Fetch file edits for diff review.
pub async fn fetch_file_edits(session_id: &str) -> Result<FileEditsResponse, ApiError> {
    let path = format!(
        "/session/{}/file-edits",
        js_sys::encode_uri_component(session_id),
    );
    api_fetch(&path).await
}

/// Cross-session message search.
pub async fn search_messages(
    project_idx: usize,
    query: &str,
    limit: Option<usize>,
) -> Result<SearchResponse, ApiError> {
    let lim = limit.unwrap_or(50);
    let path = format!(
        "/project/{}/search?q={}&limit={}",
        project_idx,
        js_sys::encode_uri_component(query),
        lim,
    );
    api_fetch(&path).await
}
