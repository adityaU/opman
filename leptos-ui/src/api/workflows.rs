//! Workflows API — matches React `api/workflows.ts`.
//! Covers routines, delegation, and workspace snapshots.

use serde::Serialize;
use crate::types::api::{
    RoutineDefinition, RoutinesListResponse, DelegatedWorkItem,
    DelegatedWorkListResponse, WorkspaceSnapshot, WorkspacesListResponse,
};
use super::client::{api_fetch, api_post, api_delete, api_patch, ApiError};

// ── Request types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    pub trigger: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_expr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateRoutineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_expr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRoutineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateDelegationRequest {
    pub title: String,
    pub assignee: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateDelegationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

// ── Routines API ────────────────────────────────────────────────────

/// Fetch all routines and their run history.
pub async fn fetch_routines() -> Result<RoutinesListResponse, ApiError> {
    api_fetch("/routines").await
}

/// Create a new routine.
pub async fn create_routine(req: &CreateRoutineRequest) -> Result<RoutineDefinition, ApiError> {
    api_post("/routines", req).await
}

/// Delete a routine.
pub async fn delete_routine(routine_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/routines/{}",
        js_sys::encode_uri_component(routine_id),
    );
    api_delete(&path).await
}

/// Update a routine.
pub async fn update_routine(routine_id: &str, req: &UpdateRoutineRequest) -> Result<RoutineDefinition, ApiError> {
    let path = format!(
        "/routines/{}",
        js_sys::encode_uri_component(routine_id),
    );
    api_patch(&path, req).await
}

/// Manually trigger a routine.
pub async fn run_routine(routine_id: &str, req: Option<&RunRoutineRequest>) -> Result<serde_json::Value, ApiError> {
    let path = format!(
        "/routines/{}/run",
        js_sys::encode_uri_component(routine_id),
    );
    let empty = serde_json::json!({});
    match req {
        Some(r) => api_post(&path, r).await,
        None => api_post(&path, &empty).await,
    }
}

// ── Delegation API ──────────────────────────────────────────────────

/// Fetch delegated work items.
pub async fn fetch_delegated_work() -> Result<Vec<DelegatedWorkItem>, ApiError> {
    let resp: DelegatedWorkListResponse = api_fetch("/delegation").await?;
    Ok(resp.items)
}

/// Create a delegated work item.
pub async fn create_delegated_work(req: &CreateDelegationRequest) -> Result<DelegatedWorkItem, ApiError> {
    api_post("/delegation", req).await
}

/// Delete a delegated work item.
pub async fn delete_delegated_work(item_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/delegation/{}",
        js_sys::encode_uri_component(item_id),
    );
    api_delete(&path).await
}

/// Update a delegated work item.
pub async fn update_delegated_work(item_id: &str, req: &UpdateDelegationRequest) -> Result<DelegatedWorkItem, ApiError> {
    let path = format!(
        "/delegation/{}",
        js_sys::encode_uri_component(item_id),
    );
    api_patch(&path, req).await
}

// ── Workspaces API ──────────────────────────────────────────────────

/// Fetch workspace snapshots.
pub async fn fetch_workspaces() -> Result<Vec<WorkspaceSnapshot>, ApiError> {
    let resp: WorkspacesListResponse = api_fetch("/workspaces").await?;
    Ok(resp.workspaces)
}

/// Save a workspace snapshot.
pub async fn save_workspace(snapshot: &WorkspaceSnapshot) -> Result<WorkspaceSnapshot, ApiError> {
    api_post("/workspaces", snapshot).await
}

/// Delete a workspace snapshot.
pub async fn delete_workspace(name: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        name: &'a str,
    }
    super::client::api_delete_with_body("/workspaces", &Body { name }).await
}
