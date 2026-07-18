use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::app::BackgroundEvent;

/// Compute the set of sessions the server currently considers active (non-idle)
/// from a `session_id → status_type` map. Idle sessions (and any absent from the
/// map) are excluded. Pure helper extracted from `spawn_session_poller`.
pub(crate) fn compute_server_active(
    status_map: &std::collections::HashMap<String, String>,
) -> HashSet<String> {
    status_map
        .iter()
        .filter(|(_, status)| status.as_str() != "idle")
        .map(|(id, _)| id.clone())
        .collect()
}

/// Given the freshly-observed active set and the previously-known active set,
/// return `(newly_busy, newly_idle)` session ids to emit transitions for.
/// Pure helper extracted from `spawn_session_poller`.
pub(crate) fn session_transitions(
    server_active: &HashSet<String>,
    known_active: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let newly_busy = server_active
        .iter()
        .filter(|id| !known_active.contains(*id))
        .cloned()
        .collect();
    let newly_idle = known_active
        .iter()
        .filter(|id| !server_active.contains(*id))
        .cloned()
        .collect();
    (newly_busy, newly_idle)
}

/// Find the largest `limit.context` value across all providers/models in a
/// `/provider` response body. Returns 0 when none is found. Pure helper
/// extracted from `spawn_provider_fetcher`.
pub(crate) fn max_context_window(body: &serde_json::Value) -> u64 {
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
    max_context
}

/// Perform a single session-status poll: fetch the status map, compute the
/// active set, emit `SseSessionBusy`/`SseSessionIdle` transitions relative to
/// `known_active`, and return the freshly-observed active set. Returns `None`
/// when the fetch failed (the caller should keep the previous `known_active`).
/// Pure-ish helper extracted from `spawn_session_poller`'s loop body so the
/// fetch-success/transition path is unit-testable against a mock upstream.
pub(crate) async fn poll_session_status_once(
    client: &crate::api::ApiClient,
    base_url: &str,
    project_dir: &str,
    known_active: &HashSet<String>,
    tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
) -> Option<HashSet<String>> {
    // Use the authoritative /session/status endpoint.  It returns a map
    // of session_id → { type: "busy"|"retry"|… } for non-idle sessions.
    // Idle sessions are absent from the map.  This correctly reflects
    // in-progress tool calls (including long-running ones) for both
    // parent and child sessions.
    let status_map = match client.fetch_session_status(base_url, project_dir).await {
        Ok(m) => m,
        Err(_) => return None,
    };

    // Sessions that the server considers active right now.
    let server_active = compute_server_active(&status_map);

    let (newly_busy, newly_idle) = session_transitions(&server_active, known_active);

    // Detect newly-busy sessions → emit SseSessionBusy.
    for id in &newly_busy {
        info!(
            project_idx,
            session_id = %id,
            "Poller: session became busy (server status)"
        );
        let _ = tx.send(BackgroundEvent::SseSessionBusy {
            session_id: id.clone(),
        });
    }

    // Detect sessions that went idle → emit SseSessionIdle.
    for id in &newly_idle {
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

    Some(server_active)
}

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

            if let Some(server_active) = poll_session_status_once(
                &client,
                &base_url,
                &project_dir,
                &known_active,
                &tx,
                project_idx,
            )
            .await
            {
                known_active = server_active;
            }
        }
    });
}

/// One provider-fetch attempt: GET `/provider`, parse the body, and return the
/// largest context window found (when positive). Returns `None` on a network
/// error, non-2xx status, malformed body, or when no positive context window is
/// present (caller retries / falls back). Extracted from `spawn_provider_fetcher`
/// so the fetch-success/parse path is unit-testable against a mock upstream.
pub(crate) async fn fetch_provider_limits_once(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
) -> Option<u64> {
    let resp = client
        .get(format!("{}/provider", base_url))
        .header("x-opencode-directory", project_dir)
        .send()
        .await;

    let body: serde_json::Value = match resp {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(_) => return None,
        },
        _ => return None,
    };

    // Find the largest context window across all providers/models
    let max_context = max_context_window(&body);

    if max_context > 0 {
        Some(max_context)
    } else {
        None
    }
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

            if let Some(max_context) =
                fetch_provider_limits_once(&client, base_url, &project_dir).await
            {
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

#[cfg(test)]
#[path = "pollers_tests.rs"]
mod pollers_tests;

#[cfg(test)]
#[path = "pollers_once_tests.rs"]
mod pollers_once_tests;
