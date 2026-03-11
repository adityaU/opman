//! Computed / intelligence types — response structs.
//!
//! These types represent the server-computed responses returned by the
//! intelligence endpoints.

use serde::Serialize;

use super::computed::{
    InboxItemPriority, InboxItemSource, InboxItemState, RecommendationAction, SignalInput,
};

// ─── Inbox ──────────────────────────────────────────────────────────

/// Unified inbox entry computed by the backend.
#[derive(Debug, Clone, Serialize)]
pub struct InboxItem {
    pub id: String,
    pub source: InboxItemSource,
    pub title: String,
    pub description: String,
    pub priority: InboxItemPriority,
    pub state: InboxItemState,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission_id: Option<String>,
}

/// Response for `POST /api/inbox`.
#[derive(Debug, Clone, Serialize)]
pub struct InboxResponse {
    pub items: Vec<InboxItem>,
}

// ─── Recommendations ────────────────────────────────────────────────

/// A single assistant recommendation.
#[derive(Debug, Clone, Serialize)]
pub struct AssistantRecommendation {
    pub id: String,
    pub title: String,
    pub rationale: String,
    pub action: RecommendationAction,
    pub priority: InboxItemPriority,
}

/// Response for `POST /api/recommendations`.
#[derive(Debug, Clone, Serialize)]
pub struct RecommendationsResponse {
    pub recommendations: Vec<AssistantRecommendation>,
}

// ─── Handoffs ───────────────────────────────────────────────────────

/// A link within a handoff brief.
#[derive(Debug, Clone, Serialize)]
pub struct HandoffLink {
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

/// A handoff brief for a mission or session.
#[derive(Debug, Clone, Serialize)]
pub struct HandoffBrief {
    pub title: String,
    pub summary: String,
    pub blockers: Vec<String>,
    pub recent_changes: Vec<String>,
    pub next_action: String,
    pub links: Vec<HandoffLink>,
}

// ─── Resume Briefing ────────────────────────────────────────────────

/// Resume briefing computed by the backend.
#[derive(Debug, Clone, Serialize)]
pub struct ResumeBriefing {
    pub title: String,
    pub summary: String,
    pub next_action: String,
}

// ─── Daily Summary ──────────────────────────────────────────────────

/// Response for `POST /api/daily-summary`.
#[derive(Debug, Clone, Serialize)]
pub struct DailySummaryResponse {
    pub summary: String,
}

// ─── Signals ────────────────────────────────────────────────────────

/// Response for `GET /api/signals`.
#[derive(Debug, Clone, Serialize)]
pub struct SignalsResponse {
    pub signals: Vec<SignalInput>,
}

// ─── Assistant Center Stats ─────────────────────────────────────────

/// Backend-computed assistant dashboard stats.
#[derive(Debug, Clone, Serialize)]
pub struct AssistantCenterStats {
    pub active_missions: usize,
    pub paused_missions: usize,
    pub total_missions: usize,
    pub pending_permissions: usize,
    pub pending_questions: usize,
    pub memory_items: usize,
    pub active_routines: usize,
    pub active_delegations: usize,
    pub workspace_count: usize,
    pub autonomy_mode: String,
}

// ─── Workspace Templates ────────────────────────────────────────────

/// A workspace template definition (backend-owned).
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub panels: super::WorkspacePanels,
    pub layout: super::WorkspaceLayout,
}

/// Response for `GET /api/workspace-templates`.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceTemplatesResponse {
    pub templates: Vec<WorkspaceTemplate>,
}
