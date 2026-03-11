use crate::app::SessionInfo;

use super::super::types::*;

/// Process a single SSE event from the opencode server.
/// We care about:
/// - `message.updated` → session stats (cost, tokens)
/// - `session.created` → add to sessions list
/// - `session.updated` → update session metadata
/// - `session.deleted` → remove from sessions list
/// - `session.status` → busy/idle tracking + watcher triggers
/// - `file.edited` → file edit tracking for diff review
pub(super) async fn handle_web_sse_event(
    handle: &super::WebStateHandle,
    data: &str,
    project_dir: &str,
) {
    #[derive(serde::Deserialize)]
    struct SseEvent {
        #[serde(rename = "type")]
        event_type: String,
        properties: serde_json::Value,
    }

    /// The opencode server wraps events in `{directory, payload}` envelope.
    #[derive(serde::Deserialize)]
    struct SseEnvelope {
        #[allow(dead_code)]
        directory: Option<String>,
        payload: SseEvent,
    }

    // Try parsing as envelope first, then as bare event.
    let event: SseEvent = if let Ok(envelope) = serde_json::from_str::<SseEnvelope>(data) {
        envelope.payload
    } else if let Ok(bare) = serde_json::from_str::<SseEvent>(data) {
        bare
    } else {
        return;
    };

    let inner = &handle.inner;
    let event_tx = &handle.event_tx;

    match event.event_type.as_str() {
        "message.updated" => {
            // Extract cost/token info
            if let Some(info) = event.properties.get("info") {
                let session_id = info
                    .get("sessionID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if session_id.is_empty() {
                    return;
                }

                let cost = info.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let tokens = info
                    .get("tokens")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                let reasoning = tokens
                    .get("reasoning")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let cache = tokens
                    .get("cache")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let cache_read = cache.get("read").and_then(|v| v.as_u64()).unwrap_or(0);
                let cache_write = cache.get("write").and_then(|v| v.as_u64()).unwrap_or(0);

                let stats = WebSessionStats {
                    cost,
                    input_tokens: input,
                    output_tokens: output,
                    reasoning_tokens: reasoning,
                    cache_read,
                    cache_write,
                };

                // Update stored stats
                {
                    let mut state = inner.write().await;
                    state
                        .session_stats
                        .insert(session_id.clone(), stats.clone());
                }

                let _ = event_tx.send(WebEvent::StatsUpdated(stats));
            }
        }
        "session.created" => {
            if let Some(info) = event.properties.get("info") {
                if let Ok(session) = serde_json::from_value::<SessionInfo>(info.clone()) {
                    let mut state = inner.write().await;

                    // Track parent→child relationships for watcher suppression
                    if !session.parent_id.is_empty() {
                        state
                            .session_children
                            .entry(session.parent_id.clone())
                            .or_default()
                            .insert(session.id.clone());
                    }

                    // Find the project by directory match
                    let dir = &session.directory;
                    for project in &mut state.projects {
                        if project.path.to_string_lossy() == dir.as_str() || dir.is_empty() {
                            // Avoid duplicates
                            if !project.sessions.iter().any(|s| s.id == session.id) {
                                project.sessions.push(session.clone());
                            }
                            // Auto-activate the new session if none is active
                            if project.active_session.is_none()
                                && session.parent_id.is_empty()
                            {
                                project.active_session = Some(session.id.clone());
                            }
                            break;
                        }
                    }
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        "session.updated" => {
            if let Some(info) = event.properties.get("info") {
                if let Ok(session) = serde_json::from_value::<SessionInfo>(info.clone()) {
                    let mut state = inner.write().await;
                    for project in &mut state.projects {
                        if let Some(existing) =
                            project.sessions.iter_mut().find(|s| s.id == session.id)
                        {
                            *existing = session.clone();
                            break;
                        }
                    }
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        "session.deleted" => {
            if let Some(session_id) = event.properties.get("sessionID").and_then(|v| v.as_str()) {
                // Clean up watcher if session is deleted
                handle.delete_watcher(session_id).await;

                let mut state = inner.write().await;
                for project in &mut state.projects {
                    project.sessions.retain(|s| s.id != session_id);
                    if project.active_session.as_deref() == Some(session_id) {
                        project.active_session = None;
                    }
                }
                state.session_stats.remove(session_id);
                state.busy_sessions.remove(session_id);

                // Clean up session_children
                state.session_children.remove(session_id);
                for children in state.session_children.values_mut() {
                    children.remove(session_id);
                }

                // Clean up file edit tracking
                state.file_snapshots.remove(session_id);
                state.file_edits.remove(session_id);

                let _ = event_tx.send(WebEvent::StateChanged);
            }
        }
        "session.status" => {
            if let Ok(status_props) =
                serde_json::from_value::<SessionStatusProps>(event.properties.clone())
            {
                let sid = status_props.session_id.clone();
                {
                    let mut state = inner.write().await;
                    match status_props.status.status_type.as_str() {
                        "busy" | "retry" => {
                            if !state.busy_sessions.contains(&sid) {
                                state.busy_sessions.insert(sid.clone());
                                let _ = event_tx.send(WebEvent::SessionBusy {
                                    session_id: sid.clone(),
                                });
                            }
                        }
                        "idle" => {
                            if state.busy_sessions.remove(&sid) {
                                let _ = event_tx.send(WebEvent::SessionIdle {
                                    session_id: sid.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }

                // Watcher integration: trigger on idle, cancel on busy
                match status_props.status.status_type.as_str() {
                    "idle" => {
                        handle.try_trigger_watcher(&sid).await;
                        // Mission loop: check if a mission needs evaluation or continuation
                        handle.try_advance_mission(&sid).await;
                    }
                    "busy" | "retry" => {
                        handle.cancel_watcher_timer(&sid).await;
                    }
                    _ => {}
                }

                // Activity feed: emit status change
                let summary = match status_props.status.status_type.as_str() {
                    "busy" => "Session started processing".to_string(),
                    "idle" => "Session completed".to_string(),
                    "retry" => "Session retrying".to_string(),
                    other => format!("Session status: {other}"),
                };
                handle.push_activity_event(ActivityEventPayload {
                    session_id: sid,
                    kind: "status".to_string(),
                    summary,
                    detail: Some(status_props.status.status_type.clone()),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }).await;
            }
        }
        "file.edited" => {
            // Extract file path from properties
            if let Some(file_path) = event.properties.get("file").and_then(|v| v.as_str()) {
                // Determine active session for this project directory
                let session_id = {
                    let state = inner.read().await;
                    state
                        .projects
                        .iter()
                        .find(|p| p.path.to_string_lossy() == project_dir)
                        .and_then(|p| p.active_session.clone())
                };

                if let Some(sid) = session_id {
                    let project_path = std::path::PathBuf::from(project_dir);
                    handle
                        .record_file_edit(&sid, file_path, Some(&project_path))
                        .await;

                    // Activity feed: emit file edit event
                    handle.push_activity_event(ActivityEventPayload {
                        session_id: sid.clone(),
                        kind: "file_edit".to_string(),
                        summary: format!("Edited {file_path}"),
                        detail: Some(file_path.to_string()),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }).await;

                    // Broadcast a state change so the frontend knows a new file edit happened
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        // ── Permission & question tracking (for reload recovery) ──
        "permission.asked" => {
            let request_id = event.properties
                .get("id").or_else(|| event.properties.get("requestID"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !request_id.is_empty() {
                let mut state = inner.write().await;
                state.pending_permissions.insert(request_id, event.properties);
            }
        }
        "permission.replied" => {
            let request_id = event.properties
                .get("requestID").or_else(|| event.properties.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !request_id.is_empty() {
                let mut state = inner.write().await;
                state.pending_permissions.remove(&request_id);
            }
        }
        "question.asked" => {
            let request_id = event.properties
                .get("id").or_else(|| event.properties.get("requestID"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !request_id.is_empty() {
                let mut state = inner.write().await;
                state.pending_questions.insert(request_id, event.properties);
            }
        }
        "question.replied" | "question.rejected" => {
            let request_id = event.properties
                .get("requestID").or_else(|| event.properties.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !request_id.is_empty() {
                let mut state = inner.write().await;
                state.pending_questions.remove(&request_id);
            }
        }
        _ => {
            // Ignore other event types
        }
    }
}

/// SSE status properties (reused from main SSE module pattern).
#[derive(serde::Deserialize)]
struct SessionStatusProps {
    #[serde(rename = "sessionID")]
    session_id: String,
    status: SessionStatusInfo,
}

#[derive(serde::Deserialize)]
struct SessionStatusInfo {
    #[serde(rename = "type")]
    status_type: String,
}
