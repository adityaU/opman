use tracing::warn;

use crate::api::ApiClient;
use crate::app::base_url;
use crate::config::{Config, ProjectEntry};

use super::super::types::*;
use super::WebProject;

impl super::WebStateHandle {
    // ── Mutations ───────────────────────────────────────────────────

    /// Broadcast a toast notification to all connected web clients.
    pub fn broadcast_toast(&self, message: String, level: &str) {
        let _ = self.event_tx.send(WebEvent::Toast {
            message,
            level: level.to_string(),
        });
    }

    /// Set the theme pair (dark + light) and broadcast a `ThemeChanged` event to SSE clients.
    pub async fn set_theme(&self, theme: WebThemePair) {
        {
            let mut inner = self.inner.write().await;
            inner.theme = Some(theme.clone());
        }
        let _ = self.event_tx.send(WebEvent::ThemeChanged(theme));
    }

    /// Switch the active project.
    pub async fn switch_project(&self, index: usize) -> bool {
        let mut inner = self.inner.write().await;
        if index < inner.projects.len() {
            inner.active_project = index;
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
        }
    }

    /// Add a new project. Returns `Ok((index, name))` on success.
    ///
    /// Validates the path is a directory, checks for duplicates, adds to the
    /// in-memory state and persists to the config file.
    pub async fn add_project(&self, path_str: &str, name: Option<&str>) -> Result<(usize, String), String> {
        let path = std::path::PathBuf::from(path_str);

        // Canonicalize the path
        let canonical = std::fs::canonicalize(&path)
            .map_err(|e| format!("Invalid path: {e}"))?;

        if !canonical.is_dir() {
            return Err("Path is not a directory".into());
        }

        // Check for duplicates
        {
            let inner = self.inner.read().await;
            for project in &inner.projects {
                if project.path == canonical {
                    return Err("Project already exists".into());
                }
            }
        }

        // Derive name from directory if not provided
        let project_name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.to_string()),
        };

        // Add to in-memory state
        let index = {
            let mut inner = self.inner.write().await;
            let project = WebProject {
                name: project_name.clone(),
                path: canonical.clone(),
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            };
            inner.projects.push(project);
            inner.projects.len() - 1
        };

        // Persist to config file
        if let Ok(mut config) = Config::load() {
            config.projects.push(ProjectEntry {
                name: project_name.clone(),
                path: canonical.to_string_lossy().to_string(),
                terminal_command: None,
            });
            if let Err(e) = config.save() {
                warn!("Failed to save config after adding project: {e}");
            }
        }

        let _ = self.event_tx.send(WebEvent::StateChanged);
        Ok((index, project_name))
    }

    /// Remove a project by index. Returns `Ok(())` on success.
    ///
    /// Removes from in-memory state and persists to the config file.
    pub async fn remove_project(&self, index: usize) -> Result<(), String> {
        {
            let mut inner = self.inner.write().await;
            if index >= inner.projects.len() {
                return Err("Invalid project index".into());
            }
            if inner.projects.len() <= 1 {
                return Err("Cannot remove the last project".into());
            }
            inner.projects.remove(index);
            // Adjust active project if needed
            if inner.active_project >= inner.projects.len() {
                inner.active_project = inner.projects.len() - 1;
            }
        }

        // Persist to config file
        if let Ok(mut config) = Config::load() {
            if index < config.projects.len() {
                config.projects.remove(index);
                if let Err(e) = config.save() {
                    warn!("Failed to save config after removing project: {e}");
                }
            }
        }

        let _ = self.event_tx.send(WebEvent::StateChanged);
        Ok(())
    }

    /// Select a session within a project (tells the opencode server too).
    pub async fn select_session(&self, project_idx: usize, session_id: String) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(project) = inner.projects.get_mut(project_idx) {
            // Verify session exists
            if project.sessions.iter().any(|s| s.id == session_id) {
                project.active_session = Some(session_id.clone());
                // Clear unseen state when user views this session
                if inner.unseen_sessions.remove(&session_id).is_some() {
                    let _ = self.event_tx.send(WebEvent::SessionSeen {
                        session_id: session_id.clone(),
                    });
                }
                drop(inner); // Release lock before async API call

                // Tell the opencode server about the selection
                let base = base_url().to_string();
                let client = ApiClient::new();
                let dir = {
                    let inner = self.inner.read().await;
                    inner
                        .projects
                        .get(project_idx)
                        .map(|p| p.path.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                if let Err(e) = client.select_session(&base, &dir, &session_id).await {
                    warn!("Failed to select session via API: {}", e);
                }

                let _ = self.event_tx.send(WebEvent::StateChanged);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Mark a session as seen (clear unseen state). Returns true if it was unseen.
    pub async fn mark_session_seen(&self, session_id: &str) -> bool {
        let mut inner = self.inner.write().await;
        if inner.unseen_sessions.remove(session_id).is_some() {
            let _ = self.event_tx.send(WebEvent::SessionSeen {
                session_id: session_id.to_string(),
            });
            true
        } else {
            false
        }
    }

    /// Add a new session to a project and activate it immediately.
    pub async fn add_and_activate_session(
        &self,
        project_idx: usize,
        session_info: crate::app::SessionInfo,
    ) {
        let mut inner = self.inner.write().await;
        if let Some(project) = inner.projects.get_mut(project_idx) {
            let session_id = session_info.id.clone();
            // Only add if not already present (avoid duplicates).
            if !project.sessions.iter().any(|s| s.id == session_id) {
                project.sessions.push(session_info);
            }
            project.active_session = Some(session_id);
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
    }

    /// Toggle a panel's visibility.
    pub async fn toggle_panel(&self, panel: &str) -> bool {
        let mut inner = self.inner.write().await;
        let ok = match panel {
            "Sidebar" | "sidebar" => {
                inner.panels.sidebar = !inner.panels.sidebar;
                true
            }
            "TerminalPane" | "terminal_pane" => {
                inner.panels.terminal_pane = !inner.panels.terminal_pane;
                true
            }
            "NeovimPane" | "neovim_pane" => {
                inner.panels.neovim_pane = !inner.panels.neovim_pane;
                true
            }
            "IntegratedTerminal" | "integrated_terminal" => {
                inner.panels.integrated_terminal = !inner.panels.integrated_terminal;
                true
            }
            "GitPanel" | "git_panel" => {
                inner.panels.git_panel = !inner.panels.git_panel;
                true
            }
            _ => false,
        };
        if ok {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
        ok
    }

    /// Set the focused panel.
    pub async fn focus_panel(&self, panel: &str) -> bool {
        let valid = matches!(
            panel,
            "Sidebar"
                | "sidebar"
                | "TerminalPane"
                | "terminal_pane"
                | "NeovimPane"
                | "neovim_pane"
                | "IntegratedTerminal"
                | "integrated_terminal"
                | "GitPanel"
                | "git_panel"
        );
        if valid {
            let mut inner = self.inner.write().await;
            // Normalize to PascalCase
            inner.focused = match panel {
                "sidebar" => "Sidebar",
                "terminal_pane" => "TerminalPane",
                "neovim_pane" => "NeovimPane",
                "integrated_terminal" => "IntegratedTerminal",
                "git_panel" => "GitPanel",
                other => other,
            }
            .to_string();
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
        }
    }

}
