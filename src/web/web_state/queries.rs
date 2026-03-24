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
            projects,
            active_project: inner.active_project,
            panels: inner.panels.clone(),
            focused: inner.focused.clone(),
            instance_name: None,
        }
    }

    /// Get session stats for a given session ID.
    pub async fn get_session_stats(&self, session_id: &str) -> Option<WebSessionStats> {
        let inner = self.inner.read().await;
        inner.session_stats.get(session_id).cloned()
    }

    /// Build a flat overview of all sessions across all projects.
    pub async fn get_sessions_overview(&self) -> SessionsOverviewResponse {
        let inner = self.inner.read().await;
        let mut sessions = Vec::new();
        let mut busy_count = 0usize;

        for (i, project) in inner.projects.iter().enumerate() {
            for s in &project.sessions {
                let is_busy = inner.busy_sessions.contains(&s.id);
                if is_busy {
                    busy_count += 1;
                }
                sessions.push(SessionOverviewEntry {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    parent_id: s.parent_id.clone(),
                    project_name: project.name.clone(),
                    project_index: i,
                    directory: s.directory.clone(),
                    is_busy,
                    time: WebSessionTime {
                        created: s.time.created,
                        updated: s.time.updated,
                    },
                    stats: inner.session_stats.get(&s.id).cloned(),
                });
            }
        }

        // Sort by most recently updated first
        sessions.sort_by(|a, b| b.time.updated.cmp(&a.time.updated));
        let total = sessions.len();
        SessionsOverviewResponse {
            sessions,
            total,
            busy_count,
        }
    }

    /// Build a tree of sessions showing parent/child relationships.
    pub async fn get_sessions_tree(&self) -> SessionsTreeResponse {
        let inner = self.inner.read().await;

        // Collect all sessions with their metadata
        struct FlatSession {
            id: String,
            title: String,
            parent_id: String,
            project_name: String,
            project_index: usize,
            is_busy: bool,
            stats: Option<WebSessionStats>,
        }

        let mut all: Vec<FlatSession> = Vec::new();
        for (i, project) in inner.projects.iter().enumerate() {
            for s in &project.sessions {
                all.push(FlatSession {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    parent_id: s.parent_id.clone(),
                    project_name: project.name.clone(),
                    project_index: i,
                    is_busy: inner.busy_sessions.contains(&s.id),
                    stats: inner.session_stats.get(&s.id).cloned(),
                });
            }
        }

        let total = all.len();
        let id_set: std::collections::HashSet<&str> =
            all.iter().map(|s| s.id.as_str()).collect();

        // Build children lookup: parent_id -> [child indices]
        let mut children_map: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut root_indices = Vec::new();

        for (idx, s) in all.iter().enumerate() {
            if s.parent_id.is_empty() || !id_set.contains(s.parent_id.as_str()) {
                root_indices.push(idx);
            } else {
                children_map
                    .entry(s.parent_id.clone())
                    .or_default()
                    .push(idx);
            }
        }

        // Recursive tree builder
        fn build_node(
            idx: usize,
            all: &[FlatSession],
            children_map: &std::collections::HashMap<String, Vec<usize>>,
        ) -> SessionTreeNode {
            let s = &all[idx];
            let children = children_map
                .get(&s.id)
                .map(|child_idxs| {
                    child_idxs
                        .iter()
                        .map(|&ci| build_node(ci, all, children_map))
                        .collect()
                })
                .unwrap_or_default();
            SessionTreeNode {
                id: s.id.clone(),
                title: s.title.clone(),
                project_name: s.project_name.clone(),
                project_index: s.project_index,
                is_busy: s.is_busy,
                stats: s.stats.clone(),
                children,
            }
        }

        let roots: Vec<SessionTreeNode> = root_indices
            .iter()
            .map(|&ri| build_node(ri, &all, &children_map))
            .collect();

        SessionsTreeResponse { roots, total }
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
        inner.projects.iter().map(|p| p.path.to_string_lossy().to_string()).collect()
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
