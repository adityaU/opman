//! Intelligence/Assistant API — matches React `api/intelligence.ts`.

use serde::Serialize;
use crate::types::api::{
    InboxResponse, RecommendationsResponse, AssistantCenterStats,
};
use super::client::{api_fetch, api_post, ApiError};

// ── Request types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PermissionInput {
    pub id: String,
    pub session_id: String,
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionInput {
    pub id: String,
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalInput {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatcherStatusInput {
    pub session_id: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_since_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InboxComputeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<PermissionInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions: Option<Vec<QuestionInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signals: Option<Vec<SignalInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watchers: Option<Vec<WatcherStatusInput>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HandoffRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResumeBriefingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailySummaryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddSignalRequest {
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantStatsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
}

// ── Response types ──────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HandoffLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HandoffBrief {
    pub session_id: String,
    pub summary: String,
    pub context: String,
    pub next_steps: Vec<String>,
    pub links: Vec<HandoffLink>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResumeBriefing {
    pub summary: String,
    pub active_sessions: Vec<String>,
    pub pending_items: Vec<String>,
    pub suggested_next: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DailySummaryResponse {
    pub summary: String,
    pub highlights: Vec<String>,
    pub metrics: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignalEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignalsResponse {
    pub signals: Vec<SignalEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkspaceTemplate {
    pub name: String,
    pub description: String,
    pub panels: serde_json::Value,
    pub layout: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkspaceTemplatesResponse {
    pub templates: Vec<WorkspaceTemplate>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActiveMemoryResponse {
    pub items: Vec<crate::types::api::PersonalMemoryItem>,
}

// ── API functions ───────────────────────────────────────────────────

/// Compute inbox items (merging permissions, questions, signals, watchers).
pub async fn compute_inbox(req: &InboxComputeRequest) -> Result<InboxResponse, ApiError> {
    api_post("/inbox", req).await
}

/// Compute assistant recommendations.
pub async fn compute_recommendations(req: &RecommendationsRequest) -> Result<RecommendationsResponse, ApiError> {
    api_post("/recommendations", req).await
}

/// Compute session handoff briefing.
pub async fn compute_session_handoff(req: &HandoffRequest) -> Result<HandoffBrief, ApiError> {
    api_post("/handoff/session", req).await
}

/// Compute resume briefing.
pub async fn compute_resume_briefing(req: &ResumeBriefingRequest) -> Result<ResumeBriefing, ApiError> {
    api_post("/resume-briefing", req).await
}

/// Compute daily summary.
pub async fn compute_daily_summary(req: &DailySummaryRequest) -> Result<DailySummaryResponse, ApiError> {
    api_post("/daily-summary", req).await
}

/// Fetch signals.
pub async fn fetch_signals() -> Result<SignalsResponse, ApiError> {
    api_fetch("/signals").await
}

/// Add a signal.
pub async fn add_signal(req: &AddSignalRequest) -> Result<SignalEntry, ApiError> {
    api_post("/signals", req).await
}

/// Compute assistant center stats.
pub async fn compute_assistant_stats(req: &AssistantStatsRequest) -> Result<AssistantCenterStats, ApiError> {
    api_post("/assistant-center/stats", req).await
}

/// Fetch workspace templates.
pub async fn fetch_workspace_templates() -> Result<Vec<WorkspaceTemplate>, ApiError> {
    let resp: WorkspaceTemplatesResponse = api_fetch("/workspace-templates").await?;
    Ok(resp.templates)
}

/// Fetch active/scoped memory.
pub async fn fetch_active_memory(
    project_index: Option<usize>,
    session_id: Option<&str>,
) -> Result<Vec<crate::types::api::PersonalMemoryItem>, ApiError> {
    let mut path = "/memory/active".to_string();
    let mut sep = '?';
    if let Some(idx) = project_index {
        path.push_str(&format!("{}project_index={}", sep, idx));
        sep = '&';
    }
    if let Some(sid) = session_id {
        path.push_str(&format!("{}session_id={}", sep, js_sys::encode_uri_component(sid)));
    }
    let resp: ActiveMemoryResponse = api_fetch(&path).await?;
    Ok(resp.items)
}

/// Compute routine summary (alias for daily-summary with routine focus).
pub async fn compute_routine_summary(req: &DailySummaryRequest) -> Result<DailySummaryResponse, ApiError> {
    api_post("/daily-summary", req).await
}
