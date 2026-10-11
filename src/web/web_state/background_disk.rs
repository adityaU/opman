//! Startup hydration from the runners' on-disk stores, for when no runner is up yet.

use super::background_sessions::merge_runner_sessions;

impl super::WebStateHandle {
    /// Fill each project's session list from what every runner has on disk.
    ///
    /// Runners start on first use, so at boot there is usually none to ask. A runner that
    /// is not started answers `sessions` from its own store without spawning anything (see
    /// `runner_lazy::LazyRunner`), so this lists every runner — the default one included —
    /// and labels each session with its owner. When a runner does start, its live list
    /// replaces the rows it owns here, and the others are left alone.
    ///
    /// The active session is deliberately not repaired: picking one would open it, and
    /// opening a session starts its runner — exactly what boot must not do on its own.
    pub(crate) async fn hydrate_from_disk(&self) {
        let Some(registry) = self.runner_registry.clone() else {
            return;
        };
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(index, project)| (index, project.path.to_string_lossy().to_string()))
                .collect()
        };
        for (index, directory) in project_paths {
            let Ok(listed) = registry
                .sessions_in(&directory, crate::runner::SessionScope::All)
                .await
            else {
                continue;
            };
            let mut state = self.inner.write().await;
            let default_runner = state.default_runner.clone();
            let mut fetched = Vec::with_capacity(listed.len());
            for (kind, session) in listed {
                let label = kind.display_name();
                if label != default_runner {
                    state
                        .session_runners
                        .insert(session.id.clone(), label.to_string());
                }
                fetched.push(session);
            }
            let session_runners = state.session_runners.clone();
            if let Some(project) = state.projects.get_mut(index) {
                project.sessions = merge_runner_sessions(
                    project,
                    &default_runner,
                    fetched,
                    &session_runners,
                    None,
                );
            }
        }
    }
}
