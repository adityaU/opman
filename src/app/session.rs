use crate::app::App;
use crate::app::SessionSelectorEntry;
use crate::app::SessionSelectorState;

impl App {
    /// Open session search mode for the active project.
    pub fn open_session_search(&mut self) {
        if let Some(project) = self.projects.get(self.active_project) {
            self.session_search_all = project.sessions.clone();
            self.session_search_results = self.session_search_all.clone();
        }
        self.session_search_mode = true;
        self.session_search_buffer.clear();
        self.session_search_cursor = 0;
        self.session_search_selected = 0;
    }

    /// Open the cross-project session selector overlay.
    /// Collects ALL sessions from ALL projects, sorted by time.updated descending.
    pub fn open_session_selector(&mut self) {
        let mut entries: Vec<SessionSelectorEntry> = Vec::new();
        for (idx, project) in self.projects.iter().enumerate() {
            for session in &project.sessions {
                entries.push(SessionSelectorEntry {
                    project_name: project.name.clone(),
                    project_idx: idx,
                    session: session.clone(),
                });
            }
        }
        // Sort by time.updated descending (most recently updated first)
        entries.sort_by(|a, b| b.session.time.updated.cmp(&a.session.time.updated));
        let filtered: Vec<usize> = (0..entries.len()).collect();
        self.session_selector = Some(SessionSelectorState {
            entries,
            query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            filtered,
        });
    }

    /// Close session search mode.
    pub fn close_session_search(&mut self) {
        self.session_search_mode = false;
        self.session_search_buffer.clear();
        self.session_search_cursor = 0;
        self.session_search_all.clear();
        self.session_search_results.clear();
        self.session_search_selected = 0;
    }

    /// Update search results based on current buffer (fuzzy match on title/id).
    pub fn update_session_search(&mut self) {
        let query = self.session_search_buffer.to_lowercase();
        if query.is_empty() {
            self.session_search_results = self.session_search_all.clone();
        } else {
            self.session_search_results = self
                .session_search_all
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&query) || s.id.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        self.session_search_selected = 0;
    }

    /// Pin the currently selected search result so it shows in sidebar, return its ID.
    pub fn pin_selected_session(&mut self) -> Option<String> {
        let session = self
            .session_search_results
            .get(self.session_search_selected)?
            .clone();
        let entry = self.pinned_sessions.entry(self.active_project).or_default();
        if !entry.contains(&session.id) {
            entry.push(session.id.clone());
        }
        self.close_session_search();
        Some(session.id)
    }
}
