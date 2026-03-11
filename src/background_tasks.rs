use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::api;
use crate::app::{self, BackgroundEvent};
use crate::pty;

/// Spawn a background task to activate a project (PTY spawn).
/// Sends BackgroundEvent::PtySpawned on success.
pub(crate) fn spawn_activate_project(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_path: PathBuf,
    terminal_rows: u16,
    terminal_cols: u16,
    theme_envs: Vec<(String, String)>,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::task::spawn_blocking(move || {
        match pty::PtyInstance::spawn(
            &base_url,
            terminal_rows,
            terminal_cols,
            &project_path,
            None,
            &theme_envs,
        ) {
            Ok(pty) => {
                let _ = tx.send(BackgroundEvent::PtySpawned {
                    project_idx,
                    session_id: "__new__".to_string(),
                    pty,
                });
                let _ = tx.send(BackgroundEvent::ProjectActivated { project_idx });
            }
            Err(e) => {
                tracing::warn!(project_idx, "Background PTY spawn failed: {}", e);
            }
        }
    });
}

/// Spawn a background task to fetch sessions for all projects via the REST API.
pub(crate) fn spawn_session_fetch(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    projects: &[app::Project],
) {
    let fetch_targets: Vec<(usize, String)> = projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let dir = p.path.to_string_lossy().to_string();
            (i, dir)
        })
        .collect();

    if fetch_targets.is_empty() {
        return;
    }

    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        let mut all_busy: Vec<String> = Vec::new();
        for (project_idx, dir) in &fetch_targets {
            match client.fetch_sessions(&base_url, dir).await {
                Ok(sessions) => {
                    let _ = tx.send(BackgroundEvent::SessionsFetched {
                        project_idx: *project_idx,
                        sessions,
                    });
                }
                Err(_) => {
                    let _ = tx.send(BackgroundEvent::SessionFetchFailed {
                        project_idx: *project_idx,
                    });
                }
            }
            // Also fetch session status to bootstrap active_sessions.
            if let Ok(status_map) = client.fetch_session_status(&base_url, dir).await {
                for (session_id, status_type) in &status_map {
                    if status_type != "idle" {
                        all_busy.push(session_id.clone());
                    }
                }
            }
        }
        if !all_busy.is_empty() {
            let _ = tx.send(BackgroundEvent::SessionStatusFetched {
                busy_sessions: all_busy,
            });
        }
    });
}

/// Spawn a background task to fetch sessions for a single project via the REST API.
pub(crate) fn spawn_single_session_fetch(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        match client.fetch_sessions(&base_url, &project_dir).await {
            Ok(sessions) => {
                let _ = tx.send(BackgroundEvent::SessionsFetched {
                    project_idx,
                    sessions,
                });
            }
            Err(_) => {
                let _ = tx.send(BackgroundEvent::SessionFetchFailed { project_idx });
            }
        }
    });
}

/// Spawn a background task to select a session via the API, then respawn PTY.
pub(crate) fn spawn_session_select(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
    session_id: String,
    project_path: PathBuf,
    terminal_rows: u16,
    terminal_cols: u16,
    theme_envs: Vec<(String, String)>,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        let _ = client
            .select_session(&base_url, &project_dir, &session_id)
            .await;
        let sid_for_pty = session_id.clone();
        let _ = tx.send(BackgroundEvent::SessionSelected {
            project_idx,
            session_id,
        });

        // Respawn PTY attached to the newly selected session
        let tx2 = tx.clone();
        let url = base_url.clone();
        let path = project_path.clone();
        let sid_clone = sid_for_pty.clone();
        tokio::task::spawn_blocking(move || {
            match pty::PtyInstance::spawn(
                &url,
                terminal_rows,
                terminal_cols,
                &path,
                Some(&sid_for_pty),
                &theme_envs,
            ) {
                Ok(pty) => {
                    let _ = tx2.send(BackgroundEvent::PtySpawned {
                        project_idx,
                        session_id: sid_clone,
                        pty,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        project_idx,
                        "PTY respawn after session select failed: {}",
                        e
                    );
                }
            }
        })
        .await
        .ok();
    });
}
