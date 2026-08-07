//! Session overview, tree, stats, context window, and agent types.

use serde::Serialize;


// ── Session stats ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone, Default)]
pub struct WebSessionStats {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session_id: String,
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

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod sessions_tests;
