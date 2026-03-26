//! Memory modal helpers: scope formatting, date display, API body structs.

use crate::types::api::{PersonalMemoryItem, ProjectInfo};
use serde::Serialize;

pub const SCOPE_OPTIONS: &[&str] = &["global", "project", "session"];

pub fn format_scope(scope: &str) -> &'static str {
    match scope {
        "global" => "Global",
        "project" => "Project",
        "session" => "Session",
        _ => "Unknown",
    }
}

pub fn describe_scope(item: &PersonalMemoryItem, projects: &[ProjectInfo]) -> String {
    match item.scope.as_str() {
        "global" => "All work".to_string(),
        "project" => item
            .project_index
            .and_then(|idx| projects.get(idx))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Project scope".to_string()),
        "session" => item
            .session_id
            .as_ref()
            .map(|sid| format!("Session {}", &sid[..sid.len().min(8)]))
            .unwrap_or_else(|| "Session scope".to_string()),
        _ => "Unknown".to_string(),
    }
}

pub fn format_relative_date(iso: &str) -> String {
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

#[derive(Serialize)]
pub struct CreateMemoryBody {
    pub label: String,
    pub content: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateMemoryBody {
    pub label: String,
    pub content: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
