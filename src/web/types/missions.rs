//! Mission tracking types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Planned,
    Active,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub next_action: String,
    pub status: MissionStatus,
    pub project_index: usize,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMissionRequest {
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub next_action: String,
    #[serde(default)]
    pub status: Option<MissionStatus>,
    pub project_index: usize,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMissionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub next_action: Option<String>,
    #[serde(default)]
    pub status: Option<MissionStatus>,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissionsListResponse {
    pub missions: Vec<Mission>,
}
