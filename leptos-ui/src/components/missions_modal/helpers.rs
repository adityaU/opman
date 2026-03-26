//! Missions modal helpers: state formatting, colors, API body structs.

use serde::Serialize;

pub const STATE_ORDER: &[&str] = &[
    "executing",
    "evaluating",
    "pending",
    "paused",
    "completed",
    "failed",
    "cancelled",
];

pub fn format_state(state: &str) -> &'static str {
    match state {
        "pending" => "Pending",
        "executing" => "Executing",
        "evaluating" => "Evaluating",
        "paused" => "Paused",
        "completed" => "Completed",
        "cancelled" => "Cancelled",
        "failed" => "Failed",
        _ => "Unknown",
    }
}

pub fn state_color(state: &str) -> &'static str {
    match state {
        "executing" | "evaluating" => "var(--color-info, #5c8fff)",
        "paused" => "var(--color-warning, #e6a817)",
        "completed" => "var(--color-success, #4caf50)",
        "failed" | "cancelled" => "var(--color-error, #e05252)",
        _ => "var(--color-text-muted, #999)",
    }
}

pub fn format_verdict(verdict: &str) -> &'static str {
    match verdict {
        "achieved" => "Achieved",
        "continue" => "Continue",
        "blocked" => "Blocked",
        "failed" => "Failed",
        _ => "Unknown",
    }
}

pub fn format_relative_date(iso: &str) -> String {
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

pub fn can_perform_action(state: &str, action: &str) -> bool {
    match action {
        "start" => state == "pending",
        "pause" => state == "executing" || state == "evaluating",
        "resume" => state == "paused",
        "cancel" => matches!(state, "pending" | "executing" | "evaluating" | "paused"),
        _ => false,
    }
}

#[derive(Serialize)]
pub struct CreateMissionBody {
    pub goal: String,
    pub session_id: Option<String>,
    pub project_index: usize,
    pub max_iterations: u32,
}

#[derive(Serialize)]
pub struct MissionActionBody {
    pub action: String,
}
