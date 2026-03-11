use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::app::BackgroundEvent;

/// Spawn a background poller that fetches sessions via the REST API every 3s and
/// detects active sessions by comparing `time.updated` changes between polls.
pub fn spawn_session_poller(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    tokio::spawn(async move {
        // Track which sessions we consider active so we only emit transitions.
        let mut known_active: HashSet<String> = HashSet::new();

        let client = crate::api::ApiClient::new();
        let base_url = crate::app::base_url().to_string();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            // Use the authoritative /session/status endpoint.  It returns a map
            // of session_id → { type: "busy"|"retry"|… } for non-idle sessions.
            // Idle sessions are absent from the map.  This correctly reflects
            // in-progress tool calls (including long-running ones) for both
            // parent and child sessions.
            let status_map = match client.fetch_session_status(&base_url, &project_dir).await {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Sessions that the server considers active right now.
            let server_active: HashSet<String> = status_map
                .iter()
                .filter(|(_, status)| status != &"idle")
                .map(|(id, _)| id.clone())
                .collect();

            // Detect newly-busy sessions → emit SseSessionBusy.
            for id in &server_active {
                if !known_active.contains(id) {
                    info!(
                        project_idx,
                        session_id = %id,
                        "Poller: session became busy (server status)"
                    );
                    let _ = tx.send(BackgroundEvent::SseSessionBusy {
                        session_id: id.clone(),
                    });
                }
            }

            // Detect sessions that went idle → emit SseSessionIdle.
            for id in &known_active {
                if !server_active.contains(id) {
                    info!(
                        project_idx,
                        session_id = %id,
                        "Poller: session became idle (server status)"
                    );
                    let _ = tx.send(BackgroundEvent::SseSessionIdle {
                        project_idx,
                        session_id: id.clone(),
                    });
                }
            }

            known_active = server_active;
        }
    });
}

/// Fetch provider model limits once at startup for a project.
/// Sends ModelLimitsFetched with the max context window found across all models.
pub fn spawn_provider_fetcher(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    tokio::spawn(async move {
        let base_url = crate::app::base_url();
        let client = reqwest::Client::new();

        // Retry a few times in case the server isn't ready yet
        for attempt in 0..5 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }

            let resp = client
                .get(format!("{}/provider", base_url))
                .header("x-opencode-directory", &project_dir)
                .send()
                .await;

            let body: serde_json::Value = match resp {
                Ok(r) if r.status().is_success() => match r.json().await {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                _ => continue,
            };

            // Find the largest context window across all providers/models
            let mut max_context: u64 = 0;
            if let Some(providers) = body.as_array() {
                for provider in providers {
                    if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                        for (_model_id, model) in models {
                            if let Some(ctx) = model
                                .get("limit")
                                .and_then(|l| l.get("context"))
                                .and_then(|c| c.as_u64())
                            {
                                if ctx > max_context {
                                    max_context = ctx;
                                }
                            }
                        }
                    }
                }
            }

            if max_context > 0 {
                let _ = tx.send(BackgroundEvent::ModelLimitsFetched {
                    project_idx,
                    context_window: max_context,
                });
                debug!(project_idx, max_context, "Provider model limits fetched");
                return;
            }
        }

        // Fallback: use 200k as default
        let _ = tx.send(BackgroundEvent::ModelLimitsFetched {
            project_idx,
            context_window: 200_000,
        });
        debug!(project_idx, "Using default context window (200k)");
    });
}
