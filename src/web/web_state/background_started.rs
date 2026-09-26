use std::collections::HashSet;

use futures::future::join_all;

use super::super::types::WebEvent;
use super::background_sessions::{merge_runner_sessions, repair_active_session};

impl super::WebStateHandle {
    /// Register the session refresh that follows a lazy runner starting.
    pub(super) fn register_runner_started_hook(&self) {
        let Some(registry) = self.runner_registry.clone() else {
            return;
        };
        let handle = self.clone();
        registry.set_on_runner_started(std::sync::Arc::new(move |kind, runner| {
            let handle = handle.clone();
            let kind = kind.clone();
            let runner = runner.clone();
            tokio::spawn(async move {
                handle.refresh_started_runner(&kind, &runner).await;
            });
        }));
    }

    /// Read the sessions owned by a runner immediately after its server comes up.
    ///
    /// This is deliberately a partial refresh: the runner that just started replaces its
    /// own entries while sessions belonging to other runners remain visible. The common
    /// merge path still owns ordering, de-duplication, and active-session repair.
    pub(crate) async fn refresh_started_runner(
        &self,
        kind: &crate::runner::RunnerKind,
        runner: &std::sync::Arc<dyn crate::runner::Runner>,
    ) {
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(index, project)| (index, project.path.to_string_lossy().to_string()))
                .collect()
        };
        let fetches = project_paths.iter().map(|(index, directory)| {
            let directory = directory.clone();
            async move {
                let sessions = runner.sessions(&directory).await;
                (*index, directory, sessions)
            }
        });
        let results = join_all(fetches).await;
        let runner_label = kind.display_name().to_string();
        let default_runner = {
            let state = self.inner.read().await;
            state.default_runner.clone()
        };
        let mut changed = false;

        for (index, directory, result) in results {
            let Ok(sessions) = result else {
                continue;
            };
            let fetched: Vec<_> = sessions
                .into_iter()
                .filter(|session| session.directory == directory)
                .collect();
            let mut state = self.inner.write().await;
            let session_runners = state.session_runners.clone();
            let fetched_ids: HashSet<_> =
                fetched.iter().map(|session| session.id.clone()).collect();
            for session_id in &fetched_ids {
                state
                    .session_runners
                    .insert(session_id.clone(), runner_label.clone());
            }
            if let Some(project) = state.projects.get_mut(index) {
                let filtered = merge_runner_sessions(
                    project,
                    &default_runner,
                    fetched,
                    &session_runners,
                    Some(&runner_label),
                );
                let active_changed = repair_active_session(project, &filtered);
                let sessions_differ = project.sessions.len() != filtered.len()
                    || project
                        .sessions
                        .iter()
                        .zip(filtered.iter())
                        .any(|(old, new)| {
                            old.id != new.id
                                || old.title != new.title
                                || old.time.updated != new.time.updated
                        });
                if sessions_differ {
                    project.sessions = filtered;
                }
                changed |= sessions_differ || active_changed;
            }
        }

        if changed {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
    }
}
