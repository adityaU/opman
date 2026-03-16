//! Missions & Memory & Autonomy API — matches React `api/missions.ts`.

use serde::Serialize;
use crate::types::api::{
    Mission, MissionsListResponse, PersonalMemoryItem, PersonalMemoryListResponse,
    AutonomySettings,
};
use super::client::{api_fetch, api_post, api_delete, api_patch, ApiError};

// ── Request types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateMissionRequest {
    pub goal: String,
    pub session_id: String,
    pub project_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateMissionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateMemoryRequest {
    pub label: String,
    pub content: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateMemoryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

// ── Missions API ────────────────────────────────────────────────────

/// List all missions.
pub async fn fetch_missions() -> Result<Vec<Mission>, ApiError> {
    let resp: MissionsListResponse = api_fetch("/missions").await?;
    Ok(resp.missions)
}

/// Get a single mission.
pub async fn fetch_mission(mission_id: &str) -> Result<Mission, ApiError> {
    let path = format!(
        "/missions/{}",
        js_sys::encode_uri_component(mission_id),
    );
    api_fetch(&path).await
}

/// Create a new mission.
pub async fn create_mission(req: &CreateMissionRequest) -> Result<Mission, ApiError> {
    api_post("/missions", req).await
}

/// Update a mission.
pub async fn update_mission(mission_id: &str, req: &UpdateMissionRequest) -> Result<Mission, ApiError> {
    let path = format!(
        "/missions/{}",
        js_sys::encode_uri_component(mission_id),
    );
    api_patch(&path, req).await
}

/// Delete a mission.
pub async fn delete_mission(mission_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/missions/{}",
        js_sys::encode_uri_component(mission_id),
    );
    api_delete(&path).await
}

/// Perform a mission action (start, pause, resume, cancel).
pub async fn mission_action(mission_id: &str, action: &str) -> Result<Mission, ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        action: &'a str,
    }
    let path = format!(
        "/missions/{}/action",
        js_sys::encode_uri_component(mission_id),
    );
    api_post(&path, &Body { action }).await
}

// ── Personal Memory API ─────────────────────────────────────────────

/// List personal memory items.
pub async fn fetch_personal_memory() -> Result<Vec<PersonalMemoryItem>, ApiError> {
    let resp: PersonalMemoryListResponse = api_fetch("/memory").await?;
    Ok(resp.memory)
}

/// Create a personal memory item.
pub async fn create_personal_memory(req: &CreateMemoryRequest) -> Result<PersonalMemoryItem, ApiError> {
    api_post("/memory", req).await
}

/// Update a personal memory item.
pub async fn update_personal_memory(id: &str, req: &UpdateMemoryRequest) -> Result<PersonalMemoryItem, ApiError> {
    let path = format!(
        "/memory/{}",
        js_sys::encode_uri_component(id),
    );
    api_patch(&path, req).await
}

/// Delete a personal memory item.
pub async fn delete_personal_memory(id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/memory/{}",
        js_sys::encode_uri_component(id),
    );
    api_delete(&path).await
}

// ── Autonomy API ────────────────────────────────────────────────────

/// Fetch autonomy settings.
pub async fn fetch_autonomy_settings() -> Result<AutonomySettings, ApiError> {
    api_fetch("/autonomy").await
}

/// Update autonomy mode.
pub async fn update_autonomy_settings(mode: &str) -> Result<AutonomySettings, ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        mode: &'a str,
    }
    api_post("/autonomy", &Body { mode }).await
}
