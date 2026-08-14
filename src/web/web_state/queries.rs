use std::collections::HashSet;
use std::path::PathBuf;

use super::super::types::*;
use super::session_page::{slice_sessions, SessionSlice, SESSION_PAGE};
use super::WebStateInner;

/// Map a stored session onto its wire shape, resolving the runner that owns it.
fn web_session(session: &crate::app::SessionInfo, inner: &WebStateInner) -> WebSessionInfo {
    WebSessionInfo {
        id: session.id.clone(),
        title: session.title.clone(),
        parent_id: session.parent_id.clone(),
        directory: session.directory.clone(),
        time: WebSessionTime {
            created: session.time.created,
            updated: session.time.updated,
        },
        runner: inner
            .session_runners
            .get(&session.id)
            .cloned()
            .unwrap_or_else(|| inner.default_runner.clone()),
        engine: session.engine.clone(),
    }
}

impl super::WebStateHandle {
    /// A page of one project's sessions, for the sidebar's "show more".
    ///
    /// `ids`, when non-empty, overrides paging and returns exactly those sessions —
    /// how the client re-hydrates pinned or open rows that are older than page one.
    pub async fn session_slice(
        &self,
        project: usize,
        offset: usize,
        limit: usize,
        ids: &[String],
    ) -> Option<WebSessionPage> {
        let inner = self.inner.read().await;
        let sessions = &inner.projects.get(project)?.sessions;
        let slice = if ids.is_empty() {
            SessionSlice::Page { offset, limit }
        } else {
            SessionSlice::Ids(ids)
        };
        let sliced = slice_sessions(sessions, slice, &HashSet::new());
        Some(WebSessionPage {
            sessions: sliced
                .sessions
                .into_iter()
                .map(|s| web_session(s, &inner))
                .collect(),
            session_count: sliced.total,
        })
    }

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
                // A session the user needs to see — active, running, failed, waiting
                // on input, or unread — stays on the first page however old it is.
                let mut keep: HashSet<&str> = busy
                    .iter()
                    .chain(&errors)
                    .chain(&inputs)
                    .chain(&unseen)
                    .map(String::as_str)
                    .collect();
                keep.extend(p.active_session.as_deref());
                let page = slice_sessions(
                    &p.sessions,
                    SessionSlice::Page {
                        offset: 0,
                        limit: SESSION_PAGE,
                    },
                    &keep,
                );

                WebProjectInfo {
                    name: p.name.clone(),
                    path: p.path.to_string_lossy().to_string(),
                    index: i,
                    active_session: p.active_session.clone(),
                    sessions: page
                        .sessions
                        .into_iter()
                        .map(|s| web_session(s, &inner))
                        .collect(),
                    session_count: page.total,
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

    /// The most recently updated session in a directory.
    ///
    /// The fallback for an asker whose runner could not tell it which session it belongs
    /// to — OpenCode's MCP config is process-wide, so `${session}` has nothing to resolve
    /// against and the env var never gets set. Newest-in-directory is the same guess the
    /// claude hook makes, and for the only case that reaches it (one agent, mid-turn, in
    /// one project) it is the session that asked.
    pub async fn newest_session_in(&self, directory: &str) -> Option<String> {
        let inner = self.inner.read().await;
        inner
            .projects
            .iter()
            .flat_map(|project| project.sessions.iter())
            .filter(|session| session.directory == directory)
            .max_by_key(|session| session.time.updated)
            .map(|session| session.id.clone())
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
