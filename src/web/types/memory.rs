//! Personal memory types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Global,
    Project,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMemoryItem {
    pub id: String,
    pub label: String,
    pub content: String,
    pub scope: MemoryScope,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePersonalMemoryRequest {
    pub label: String,
    pub content: String,
    pub scope: MemoryScope,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePersonalMemoryRequest {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub scope: Option<MemoryScope>,
    #[serde(default)]
    pub project_index: Option<Option<usize>>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonalMemoryListResponse {
    pub memory: Vec<PersonalMemoryItem>,
}
