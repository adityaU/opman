use crate::app::App;
use crate::app::{SessionInfo, SidebarItem};

impl App {
    /// Get the sessions to display in the sidebar for a project (max 5 latest + pinned).
    /// Only returns parent sessions (parent_id is empty).
    /// Only returns sessions if this project is the one currently expanded.
    pub fn visible_sessions(&self, project_idx: usize) -> Vec<&SessionInfo> {
        // Only one project can have sessions expanded at a time
        if self.sessions_expanded_for != Some(project_idx) {
            return Vec::new();
        }

        let project = match self.projects.get(project_idx) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let pinned = self.pinned_sessions.get(&project_idx);
        let mut visible: Vec<&SessionInfo> = Vec::new();

        // Always show pinned sessions first (only parent sessions)
        if let Some(pinned_ids) = pinned {
            for pid in pinned_ids {
                if let Some(s) = project
                    .sessions
                    .iter()
                    .find(|s| &s.id == pid && s.parent_id.is_empty())
                {
                    visible.push(s);
                }
            }
        }

        // Then latest parent sessions (up to 5 total, excluding already-visible pinned ones)
        for session in project.sessions.iter() {
            if visible.len() >= 5 {
                break;
            }
            if !session.parent_id.is_empty() {
                continue;
            }
            if !visible.iter().any(|v| v.id == session.id) {
                visible.push(session);
            }
        }
        visible
    }

    /// Get subagent sessions for a given parent session ID within a project.
    pub fn subagent_sessions(
        &self,
        project_idx: usize,
        parent_session_id: &str,
    ) -> Vec<&SessionInfo> {
        let project = match self.projects.get(project_idx) {
            Some(p) => p,
            None => return Vec::new(),
        };
        project
            .sessions
            .iter()
            .filter(|s| s.parent_id == parent_session_id)
            .collect()
    }

    /// Whether a project has more parent sessions than what's visible.
    pub fn has_more_sessions(&self, project_idx: usize) -> bool {
        if self.sessions_expanded_for != Some(project_idx) {
            return false;
        }
        self.projects
            .get(project_idx)
            .map(|p| p.sessions.iter().filter(|s| s.parent_id.is_empty()).count() > 5)
            .unwrap_or(false)
    }

    /// Map a flat sidebar_selection index to the item it represents.
    pub fn sidebar_item_at(&self, selection: usize) -> Option<SidebarItem> {
        let mut idx = 0;
        for (i, _project) in self.projects.iter().enumerate() {
            if idx == selection {
                return Some(SidebarItem::Project(i));
            }
            idx += 1;

            // "New Session" item appears when sessions are expanded
            if self.sessions_expanded_for == Some(i) {
                if idx == selection {
                    return Some(SidebarItem::NewSession(i));
                }
                idx += 1;
            }

            let visible = self.visible_sessions(i);
            for session in &visible {
                if idx == selection {
                    return Some(SidebarItem::Session(i, session.id.clone()));
                }
                idx += 1;

                // Subagent sessions under this parent
                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    let subs = self.subagent_sessions(i, &session.id);
                    for sub in &subs {
                        if idx == selection {
                            return Some(SidebarItem::SubAgentSession(i, sub.id.clone()));
                        }
                        idx += 1;
                    }
                }
            }

            if self.has_more_sessions(i) {
                if idx == selection {
                    return Some(SidebarItem::MoreSessions(i));
                }
                idx += 1;
            }
        }
        if idx == selection {
            return Some(SidebarItem::AddProject);
        }
        None
    }

    /// Total number of items in the sidebar (for navigation bounds).
    pub fn sidebar_item_count(&self) -> usize {
        let mut count = 0;
        for (i, _) in self.projects.iter().enumerate() {
            count += 1; // project
            if self.sessions_expanded_for == Some(i) {
                count += 1; // "New Session"
            }
            let vis = self.visible_sessions(i);
            for session in &vis {
                count += 1; // session
                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    count += self.subagent_sessions(i, &session.id).len();
                }
            }
            if self.has_more_sessions(i) {
                count += 1; // "more..."
            }
        }
        count += 1; // "[+ Add]"
        count
    }

    /// Compute the flat sidebar index for a given project + session ID.
    /// Returns `None` if the session is not currently visible in the sidebar.
    fn sidebar_index_for_session(&self, project_idx: usize, session_id: &str) -> Option<usize> {
        let mut idx = 0;
        for (i, _) in self.projects.iter().enumerate() {
            idx += 1; // project row

            if self.sessions_expanded_for == Some(i) {
                idx += 1; // "New Session"
            }

            let vis = self.visible_sessions(i);
            for session in &vis {
                if i == project_idx && session.id == session_id {
                    return Some(idx);
                }
                idx += 1;

                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    idx += self.subagent_sessions(i, &session.id).len();
                }
            }

            if self.has_more_sessions(i) {
                idx += 1;
            }
        }
        None
    }

    /// Keep `sidebar_selection` in sync with the active project's active
    /// session so the highlight always reflects what is shown in the
    /// terminal pane.
    pub fn sync_sidebar_to_active_session(&mut self) {
        let proj = self.active_project;
        if let Some(ref sid) = self
            .projects
            .get(proj)
            .and_then(|p| p.active_session.clone())
        {
            if let Some(flat) = self.sidebar_index_for_session(proj, sid) {
                self.sidebar_selection = flat;
            }
        }
    }
}
