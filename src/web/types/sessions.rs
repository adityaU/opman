//! Session overview, tree, stats, context window, and agent types.

use serde::Serialize;

use super::state::WebSessionTime;

// ── Session stats ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone, Default)]
pub struct WebSessionStats {
    pub cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

// ── Context Window types ────────────────────────────────────────────

/// Response for `GET /api/context-window`.
///
/// Provides a breakdown of context window usage for the active session,
/// including total limit, used tokens by category, and per-item estimates.
#[derive(Serialize, Clone, Debug)]
pub struct ContextWindowResponse {
    /// Maximum context window size in tokens for the active model.
    pub context_limit: u64,
    /// Total tokens currently used across all categories.
    pub total_used: u64,
    /// Usage percentage (0–100).
    pub usage_pct: f64,
    /// Breakdown by category.
    pub categories: Vec<ContextCategory>,
    /// Estimated messages remaining at current rate.
    pub estimated_messages_remaining: Option<u64>,
}

/// A single category of context window usage.
#[derive(Serialize, Clone, Debug)]
pub struct ContextCategory {
    /// Category name: "system", "messages", "tool_results", "files", "cache"
    pub name: String,
    /// Human-readable label.
    pub label: String,
    /// Tokens consumed by this category.
    pub tokens: u64,
    /// Percentage of total context window.
    pub pct: f64,
    /// Color hint for the frontend: "blue", "green", "orange", "purple", "gray"
    pub color: String,
    /// Individual items within this category (if available).
    pub items: Vec<ContextItem>,
}

/// An individual item contributing to context usage.
#[derive(Serialize, Clone, Debug)]
pub struct ContextItem {
    /// Item description (e.g. message preview, file path, tool name).
    pub label: String,
    /// Estimated tokens for this item.
    pub tokens: u64,
}

// ── Multi-session dashboard types ───────────────────────────────────

/// A single session entry in the sessions overview, enriched with stats and status.
#[derive(Serialize, Clone)]
pub struct SessionOverviewEntry {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub project_name: String,
    pub project_index: usize,
    pub directory: String,
    pub is_busy: bool,
    pub time: WebSessionTime,
    /// Cost and token usage (None if no stats recorded yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<WebSessionStats>,
}

/// Response for `GET /api/sessions/overview`.
#[derive(Serialize)]
pub struct SessionsOverviewResponse {
    pub sessions: Vec<SessionOverviewEntry>,
    /// Total number of sessions across all projects.
    pub total: usize,
    /// Number of currently busy sessions.
    pub busy_count: usize,
}

/// A node in the session tree (parent/child relationships).
#[derive(Serialize, Clone)]
pub struct SessionTreeNode {
    pub id: String,
    pub title: String,
    pub project_name: String,
    pub project_index: usize,
    pub is_busy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<WebSessionStats>,
    pub children: Vec<SessionTreeNode>,
}

/// Response for `GET /api/sessions/tree`.
#[derive(Serialize)]
pub struct SessionsTreeResponse {
    /// Root-level sessions (sessions without a parent, or whose parent is not known).
    pub roots: Vec<SessionTreeNode>,
    /// Total session count.
    pub total: usize,
}

// ── Agent types ─────────────────────────────────────────────────────

/// An agent entry returned by `GET /api/agents`.
///
/// Fields mirror the opencode Agent type so the frontend can filter and display
/// agents the same way opencode does (e.g. hide subagents, colour-code chips).
#[derive(Serialize, Clone)]
pub struct AgentEntry {
    pub id: String,
    pub label: String,
    pub description: String,
    /// "primary", "subagent", or "all".
    #[serde(default)]
    pub mode: String,
    /// Whether the agent should be hidden from the selector.
    #[serde(default)]
    pub hidden: bool,
    /// Whether this is a built-in agent (coder, task, etc.).
    #[serde(default)]
    pub native: bool,
    /// Optional display colour (CSS colour string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
