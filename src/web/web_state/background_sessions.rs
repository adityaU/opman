use std::collections::HashSet;

use futures::future::join_all;

use crate::api::ApiClient;
use crate::app::{base_url_ready, try_base_url};

use super::super::types::*;
use super::background_hydration::StartupHydration;

fn merge_runner_sessions(
    project: &super::WebProject,
    default_runner: &str,
    mut fetched: Vec<crate::app::SessionInfo>,
    session_runners: &std::collections::HashMap<String, String>,
) -> Vec<crate::app::SessionInfo> {
    let fetched_ids: HashSet<String> = fetched.iter().map(|session| session.id.clone()).collect();
    fetched.extend(
        project
            .sessions
            .iter()
            .filter(|session| {
                !fetched_ids.contains(&session.id)
                    && session_runners
                        .get(&session.id)
                        .is_some_and(|runner| runner != default_runner)
            })
            .cloned(),
    );
    fetched
}

fn repair_active_session(
    project: &mut super::WebProject,
    sessions: &[crate::app::SessionInfo],
) -> bool {
    if project
        .active_session
        .as_ref()
        .is_some_and(|active| sessions.iter().any(|session| session.id == *active))
    {
        return false;
    }

    let next = sessions.first().map(|session| session.id.clone());
    if project.active_session == next {
        return false;
    }
    project.active_session = next;
    true
}

impl super::WebStateHandle {
    /// Poll `GET /session` for each project every 30 seconds.
    ///
    /// On startup the poller retries aggressively (100ms → 200ms → …) so that
    /// sessions are available for the very first `/api/state` request from the
    /// frontend, eliminating the previous 2-second race window.
    pub(super) fn spawn_session_poller(&self) {
        let handle = self.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let client = ApiClient::new();
            let mut delay_ms: u64 = 0;
            for attempt in 1..=8 {
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                match handle.session_poll_startup(&client).await {
                    StartupHydration::Pending => {}
                    StartupHydration::NoRunner | StartupHydration::Complete => {
                        let _ = event_tx.send(WebEvent::StateChanged);
                        tracing::debug!("initial session hydration settled on attempt {attempt}");
                        break;
                    }
                }
                delay_ms = if delay_ms == 0 {
                    100
                } else {
                    (delay_ms * 2).min(2000)
                };
            }

            loop {
                let base = base_url_ready().await.to_string();
                handle.session_poll_iter_once(&client, &base).await;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    /// Run one eager startup attempt without waiting for a runner that has not started.
    pub(crate) async fn session_poll_startup(&self, client: &ApiClient) -> StartupHydration {
        let Some(base) = try_base_url() else {
            let mut state = self.inner.write().await;
            state.startup_hydration = StartupHydration::NoRunner;
            return StartupHydration::NoRunner;
        };
        self.session_poll_startup_once(client, base).await
    }

    /// One eager-startup poll attempt: fetch sessions for each project and hydrate
    /// `active_session` + the session list. A failed fetch stays pending so the
    /// eager retry loop can try again.
    pub(crate) async fn session_poll_startup_once(
        &self,
        client: &ApiClient,
        base: &str,
    ) -> StartupHydration {
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
                .collect()
        };
        let fetches = project_paths.iter().map(|(idx, dir)| {
            let base = base.to_string();
            let dir = dir.clone();
            async move {
                let result = client.fetch_sessions(&base, &dir).await;
                (*idx, dir, result)
            }
        });
        let results = join_all(fetches).await;
        let mut hydration = StartupHydration::Complete;
        for (idx, dir, result) in results {
            if let Ok(sessions) = result {
                let native_sessions = if let Some(registry) = &self.runner_registry {
                    registry.sessions(&dir).await.unwrap_or_default()
                } else {
                    Vec::new()
                };
                let native_labels: Vec<_> = native_sessions
                    .iter()
                    .map(|(kind, session)| (session.id.clone(), kind.display_name().to_string()))
                    .collect();
                let mut fetched: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| s.directory == dir)
                    .collect();
                fetched.extend(native_sessions.into_iter().map(|(_, session)| session));
                let mut state = self.inner.write().await;
                let default_runner = state.default_runner.clone();
                let session_runners = state.session_runners.clone();
                for (session_id, runner) in native_labels {
                    state.session_runners.insert(session_id, runner);
                }
                if let Some(project) = state.projects.get_mut(idx) {
                    let filtered =
                        merge_runner_sessions(project, &default_runner, fetched, &session_runners);
                    repair_active_session(project, &filtered);
                    project.sessions = filtered;
                }
            } else {
                hydration = StartupHydration::Pending;
            }
        }
        let mut state = self.inner.write().await;
        state.startup_hydration = hydration;
        hydration
    }

    pub(crate) async fn session_poll_iter_once(&self, client: &ApiClient, base: &str) {
        let project_paths: Vec<(usize, String)> = {
            let state = self.inner.read().await;
            state
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
                .collect()
        };

        let mut changed = false;
        let runner_registry = self.runner_registry.clone();
        let fetches = project_paths.iter().map(|(idx, dir)| {
            let base = base.to_string();
            let dir = dir.clone();
            let idx = *idx;
            let runner_registry = runner_registry.clone();
            async move {
                let sessions = client.fetch_sessions(&base, &dir).await;
                let native_sessions = if let Some(registry) = runner_registry {
                    registry.sessions(&dir).await.ok()
                } else {
                    None
                };
                (idx, dir, sessions.ok(), native_sessions)
            }
        });
        let results = join_all(fetches).await;

        for (idx, dir, sessions, native_sessions) in results {
            if let Some(sessions) = sessions {
                let mut state = self.inner.write().await;
                let default_runner = state.default_runner.clone();
                let session_runners = state.session_runners.clone();
                let native_labels = native_sessions
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .map(|(kind, session)| (session.id.clone(), kind.display_name().to_string()))
                    .collect::<Vec<_>>();
                let mut fetched: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| s.directory == dir)
                    .collect();
                if let Some(native_sessions) = native_sessions {
                    fetched.extend(native_sessions.into_iter().map(|(_, session)| session));
                }
                for (session_id, runner) in native_labels {
                    state.session_runners.insert(session_id, runner);
                }
                if let Some(project) = state.projects.get_mut(idx) {
                    let filtered =
                        merge_runner_sessions(project, &default_runner, fetched, &session_runners);
                    let active_changed = project.active_session.is_some()
                        && repair_active_session(project, &filtered);
                    let sessions_differ = if project.sessions.len() != filtered.len() {
                        true
                    } else {
                        project.sessions.iter().zip(filtered.iter()).any(|(a, b)| {
                            a.id != b.id || a.title != b.title || a.time.updated != b.time.updated
                        })
                    };
                    if sessions_differ {
                        project.sessions = filtered;
                        changed = true;
                    }
                    if active_changed {
                        changed = true;
                    }
                }
            }
        }

        if changed {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
    }
}
