use std::path::PathBuf;

use super::super::types::*;

impl super::WebStateHandle {
    // ── Queries ─────────────────────────────────────────────────────

    /// Build a complete `WebAppState` snapshot for the `/api/state` endpoint.
    pub async fn get_state(&self) -> WebAppState {
        let inner = self.inner.read().await;
        let projects = inner
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let busy: Vec<String> = p
                    .sessions
                    .iter()
                    .filter(|s| inner.busy_sessions.contains(&s.id))
                    .map(|s| s.id.clone())
                    .collect();
                let errors: Vec<String> = p
                    .sessions
                    .iter()
                    .filter(|s| inner.error_sessions.contains_key(&s.id))
                    .map(|s| s.id.clone())
                    .collect();
                let inputs: Vec<String> = p
                    .sessions
                    .iter()
                    .filter(|s| inner.input_sessions.contains(&s.id))
                    .map(|s| s.id.clone())
                    .collect();
                let unseen: Vec<String> = p
                    .sessions
                    .iter()
                    .filter(|s| inner.unseen_sessions.contains_key(&s.id))
                    .map(|s| s.id.clone())
                    .collect();
                WebProjectInfo {
                    name: p.name.clone(),
                    path: p.path.to_string_lossy().to_string(),
                    index: i,
                    active_session: p.active_session.clone(),
                    sessions: p
                        .sessions
                        .iter()
                        .map(|s| WebSessionInfo {
                            id: s.id.clone(),
                            title: s.title.clone(),
                            parent_id: s.parent_id.clone(),
                            directory: s.directory.clone(),
                            time: WebSessionTime {
                                created: s.time.created,
                                updated: s.time.updated,
                            },
                            runner: inner
                                .session_runners
                                .get(&s.id)
                                .cloned()
                                .unwrap_or_else(|| inner.default_runner.clone()),
                        })
                        .collect(),
                    git_branch: p.git_branch.clone(),
                    busy_sessions: busy,
                    error_sessions: errors,
                    input_sessions: inputs,
                    unseen_sessions: unseen,
                }
            })
            .collect();

        WebAppState {
            startup_ready: inner.startup_ready,
            projects,
            active_project: inner.active_project,
            panels: inner.panels.clone(),
            focused: inner.focused.clone(),
            instance_name: None,
            backend: String::new(),
            default_runner: inner.default_runner.clone(),
            runners: vec![],
        }
    }

    /// Whether this session has already been given its session instructions.
    pub async fn instructions_delivered(&self, session_id: &str) -> bool {
        self.inner
            .read()
            .await
            .instructions_sent
            .contains(session_id)
    }

    /// The runner label recorded for a logical session, if one is known.
    ///
    /// Unlike the `/api/state` projection this does not fall back to the default
    /// runner: callers need to tell "owned by the default runner" apart from
    /// "ownership unknown".
    pub async fn session_runner(&self, session_id: &str) -> Option<String> {
        self.inner
            .read()
            .await
            .session_runners
            .get(session_id)
            .cloned()
    }

    /// Get session stats for a given session ID.
    pub async fn get_session_stats(&self, session_id: &str) -> Option<WebSessionStats> {
        let inner = self.inner.read().await;
        inner.session_stats.get(session_id).cloned()
    }

    /// Get all tracked file edits for a session.
    pub async fn get_file_edits(&self, session_id: &str) -> Vec<super::FileEditRecord> {
        let inner = self.inner.read().await;
        inner
            .file_edits
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get the working directory of the active project.
    pub async fn get_working_dir(&self) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .projects
            .get(inner.active_project)
            .map(|p| p.path.clone())
    }

    /// Get all project paths (for the directory browser to mark existing projects).
    pub async fn all_project_paths(&self) -> Vec<String> {
        let inner = self.inner.read().await;
        inner
            .projects
            .iter()
            .map(|p| p.path.to_string_lossy().to_string())
            .collect()
    }

    pub async fn active_project_index(&self) -> usize {
        let inner = self.inner.read().await;
        inner.active_project
    }

    /// Get the working directory of a specific project by index.
    pub async fn get_project_working_dir(&self, project_idx: usize) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner.projects.get(project_idx).map(|p| p.path.clone())
    }

    /// Get all sessions for a specific project, with project metadata.
    /// Returns (project_path, project_name, Vec<(session_id, session_title)>).
    pub async fn get_project_sessions(
        &self,
        project_idx: usize,
    ) -> Option<(PathBuf, String, Vec<(String, String)>)> {
        let inner = self.inner.read().await;
        let project = inner.projects.get(project_idx)?;
        let sessions: Vec<(String, String)> = project
            .sessions
            .iter()
            .map(|s| (s.id.clone(), s.title.clone()))
            .collect();
        Some((project.path.clone(), project.name.clone(), sessions))
    }

    /// Get the current theme pair (dark + light) if set.
    pub async fn get_theme(&self) -> Option<WebThemePair> {
        let inner = self.inner.read().await;
        inner.theme.clone()
    }

    /// Get the active session ID for the active project.
    pub async fn active_session_id(&self) -> Option<String> {
        let inner = self.inner.read().await;
        inner
            .projects
            .get(inner.active_project)
            .and_then(|p| p.active_session.clone())
    }
}

#[cfg(test)]
#[path = "queries_tests.rs"]
mod queries_tests;
