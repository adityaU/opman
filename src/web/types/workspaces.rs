//! Workspace snapshot types.

use serde::{Deserialize, Serialize};

/// Saved workspace snapshot — captures the full panel/layout state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// User-chosen name for this workspace.
    pub name: String,
    /// ISO-8601 timestamp when the snapshot was created.
    pub created_at: String,
    /// Panel visibility states.
    pub panels: WorkspacePanels,
    /// Panel sizes (percentages or pixel values).
    #[serde(default)]
    pub layout: WorkspaceLayout,
    /// Paths of files that were open in the editor.
    #[serde(default)]
    pub open_files: Vec<String>,
    /// Which file was the active/focused one in the editor.
    #[serde(default)]
    pub active_file: Option<String>,
    /// Terminal tabs that were open.
    #[serde(default)]
    pub terminal_tabs: Vec<WorkspaceTerminalTab>,
    /// Active session ID when snapshot was taken.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Git branch that was checked out.
    #[serde(default)]
    pub git_branch: Option<String>,
    /// Whether this is a built-in task template (not user-deletable).
    #[serde(default)]
    pub is_template: bool,
    /// Intent-oriented recipe metadata.
    #[serde(default)]
    pub recipe_description: Option<String>,
    #[serde(default)]
    pub recipe_next_action: Option<String>,
    #[serde(default)]
    pub is_recipe: bool,
}

/// Panel visibility flags within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePanels {
    pub sidebar: bool,
    pub terminal: bool,
    pub editor: bool,
    pub git: bool,
}

/// Panel layout sizes within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceLayout {
    /// Sidebar width in pixels (0 = use default).
    #[serde(default)]
    pub sidebar_width: u32,
    /// Terminal height in pixels (0 = use default).
    #[serde(default)]
    pub terminal_height: u32,
    /// Side panel width in pixels (0 = use default).
    #[serde(default)]
    pub side_panel_width: u32,
}

/// Terminal tab descriptor within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTerminalTab {
    /// Label shown on the tab.
    pub label: String,
    /// Kind of terminal (e.g. "shell", "command").
    #[serde(default)]
    pub kind: String,
}

/// Request body for saving a workspace snapshot.
#[derive(Debug, Clone, Deserialize)]
pub struct SaveWorkspaceRequest {
    pub snapshot: WorkspaceSnapshot,
}

/// Response listing all saved workspaces.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspacesListResponse {
    pub workspaces: Vec<WorkspaceSnapshot>,
}
