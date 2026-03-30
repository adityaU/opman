//! Watcher API — matches React `api/watchers.ts`.

use serde::Serialize;
use crate::types::api::{
    WatcherConfigResponse, WatcherListEntry, WatcherSessionEntry,
    ActivityEvent, ClientPresence,
};
use super::client::{api_fetch, api_post, api_delete, api_post_void, ApiError};

// ── Request types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct WatcherConfigRequest {
    pub session_id: String,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    pub include_original: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_message: Option<String>,
    pub hang_message: String,
    pub hang_timeout_secs: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WatcherMessageEntry {
    pub id: String,
    pub text: String,
    pub timestamp: f64,
}

// Note: WatcherListResponse, WatcherSessionsResponse, WatcherMessagesResponse
// wrappers removed — backend returns raw arrays for these endpoints.

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActivityFeedResponse {
    pub events: Vec<ActivityEvent>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PresenceResponse {
    pub clients: Vec<ClientPresence>,
}

// ── API functions ───────────────────────────────────────────────────

/// List all active watchers.
pub async fn list_watchers() -> Result<Vec<WatcherListEntry>, ApiError> {
    api_fetch("/watchers").await
}

/// Get watcher config for a specific session.
pub async fn get_watcher(session_id: &str) -> Result<WatcherConfigResponse, ApiError> {
    let path = format!(
        "/watcher/{}",
        js_sys::encode_uri_component(session_id),
    );
    api_fetch(&path).await
}

/// Create or update a watcher.
pub async fn create_watcher(req: &WatcherConfigRequest) -> Result<WatcherConfigResponse, ApiError> {
    api_post("/watcher", req).await
}

/// Delete a watcher.
pub async fn delete_watcher(session_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/watcher/{}",
        js_sys::encode_uri_component(session_id),
    );
    api_delete(&path).await
}

/// Get sessions eligible for watchers.
pub async fn get_watcher_sessions() -> Result<Vec<WatcherSessionEntry>, ApiError> {
    api_fetch("/watcher/sessions").await
}

/// Get user messages for a specific session (for original message picker).
pub async fn get_watcher_messages(session_id: &str) -> Result<Vec<WatcherMessageEntry>, ApiError> {
    let path = format!(
        "/watcher/{}/messages",
        js_sys::encode_uri_component(session_id),
    );
    api_fetch(&path).await
}

/// Fetch activity feed events.
pub async fn fetch_activity_feed(session_id: Option<&str>) -> Result<Vec<ActivityEvent>, ApiError> {
    let path = match session_id {
        Some(sid) => format!(
            "/activity?session_id={}",
            js_sys::encode_uri_component(sid),
        ),
        None => "/activity".to_string(),
    };
    let resp: ActivityFeedResponse = api_fetch(&path).await?;
    Ok(resp.events)
}

/// Fetch current client presence.
pub async fn fetch_presence() -> Result<Vec<ClientPresence>, ApiError> {
    let resp: PresenceResponse = api_fetch("/presence").await?;
    Ok(resp.clients)
}

/// Register this client's presence.
pub async fn register_presence(
    client_id: &str,
    interface_type: &str,
    focused_session: Option<&str>,
) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        client_id: &'a str,
        interface_type: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        focused_session: Option<&'a str>,
    }
    api_post_void("/presence", &Body { client_id, interface_type, focused_session }).await
}

/// Deregister a client's presence.
pub async fn deregister_presence(client_id: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        client_id: &'a str,
    }
    super::client::api_delete_with_body("/presence", &Body { client_id }).await
}
