use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::app::BackgroundEvent;

use super::{
    MessageUpdatedProps, SessionCreatedProps, SessionDeletedProps, SessionStatusProps, SseEvent,
};

pub(super) fn handle_sse_data(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    data: &str,
) -> Result<()> {
    let event: SseEvent = serde_json::from_str(data)?;

    match event.event_type.as_str() {
        "session.created" => {
            let props: SessionCreatedProps = serde_json::from_value(event.properties)?;
            info!(
                project_idx,
                session_id = %props.info.id,
                title = %props.info.title,
                "SSE: session.created — new session detected"
            );
            let _ = bg_tx.send(BackgroundEvent::SseSessionCreated {
                project_idx,
                session: props.info,
            });
        }
        "session.updated" => {
            let props: SessionCreatedProps = serde_json::from_value(event.properties)?;
            debug!(project_idx, session_id = %props.info.id, "SSE: session.updated");
            let _ = bg_tx.send(BackgroundEvent::SseSessionUpdated {
                project_idx,
                session: props.info,
            });
        }
        "session.deleted" => {
            let props: SessionDeletedProps = serde_json::from_value(event.properties)?;
            info!(project_idx, session_id = %props.session_id, "SSE: session.deleted");
            let _ = bg_tx.send(BackgroundEvent::SseSessionDeleted {
                project_idx,
                session_id: props.session_id,
            });
        }
        "session.idle" => {
            let props: SessionDeletedProps = serde_json::from_value(event.properties)?;
            info!(project_idx, session_id = %props.session_id, "SSE: session.idle");
            let _ = bg_tx.send(BackgroundEvent::SseSessionIdle {
                project_idx,
                session_id: props.session_id,
            });
        }
        "session.status" => {
            let props: SessionStatusProps = serde_json::from_value(event.properties)?;
            info!(
                project_idx,
                session_id = %props.session_id,
                status = %props.status.status_type,
                "SSE: session.status — state change"
            );
            match props.status.status_type.as_str() {
                "busy" => {
                    let _ = bg_tx.send(BackgroundEvent::SseSessionBusy {
                        session_id: props.session_id,
                    });
                }
                "idle" => {
                    let _ = bg_tx.send(BackgroundEvent::SseSessionIdle {
                        project_idx,
                        session_id: props.session_id,
                    });
                }
                other => {
                    debug!(
                        project_idx,
                        status = other,
                        "SSE: session.status — unhandled type"
                    );
                }
            }
        }
        "server.connected" => {
            debug!(project_idx, "SSE: server connected");
        }
        "file.edited" => {
            info!(project_idx, raw_props = %event.properties, "SSE: file.edited raw event");
            if let Some(file) = event.properties.get("file").and_then(|v| v.as_str()) {
                debug!(project_idx, file, "SSE: file.edited - sending to handler");
                let _ = bg_tx.send(BackgroundEvent::SseFileEdited {
                    project_idx,
                    file_path: file.to_string(),
                });
            } else {
                warn!(project_idx, raw_props = %event.properties, "SSE: file.edited - no 'file' property found");
            }
        }
        "todo.updated" => {
            if let Some(session_id) = event.properties.get("sessionID").and_then(|v| v.as_str()) {
                if let Ok(todos) = serde_json::from_value::<Vec<crate::app::TodoItem>>(
                    event.properties.get("todos").cloned().unwrap_or_default(),
                ) {
                    debug!(
                        project_idx,
                        session_id,
                        count = todos.len(),
                        "SSE: todo.updated"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SseTodoUpdated {
                        session_id: session_id.to_string(),
                        todos,
                    });
                }
            }
        }
        "permission.asked" => {
            match serde_json::from_value::<crate::app::PermissionRequest>(event.properties) {
                Ok(req) => {
                    info!(
                        project_idx,
                        request_id = %req.id,
                        session_id = %req.session_id,
                        permission = %req.permission,
                        "SSE: permission.asked"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SsePermissionAsked {
                        project_idx,
                        request: req,
                    });
                }
                Err(e) => {
                    warn!(project_idx, error = %e, "SSE: permission.asked — failed to parse");
                }
            }
        }
        "question.asked" => {
            match serde_json::from_value::<crate::app::QuestionRequest>(event.properties) {
                Ok(req) => {
                    info!(
                        project_idx,
                        request_id = %req.id,
                        session_id = %req.session_id,
                        question_count = req.questions.len(),
                        "SSE: question.asked"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SseQuestionAsked {
                        project_idx,
                        request: req,
                    });
                }
                Err(e) => {
                    warn!(project_idx, error = %e, "SSE: question.asked — failed to parse");
                }
            }
        }
        _ => {
            // Try to handle message.updated for token/cost tracking
            if event.event_type == "message.updated" {
                if let Ok(props) = serde_json::from_value::<MessageUpdatedProps>(event.properties) {
                    let info = &props.info;
                    debug!(
                        session_id = %info.session_id,
                        cost = info.cost,
                        input_tokens = info.tokens.input,
                        output_tokens = info.tokens.output,
                        "SSE: message.updated with cost/token data"
                    );
                    let _ = bg_tx.send(BackgroundEvent::SseMessageUpdated {
                        session_id: info.session_id.clone(),
                        cost: info.cost,
                        input_tokens: info.tokens.input,
                        output_tokens: info.tokens.output,
                        reasoning_tokens: info.tokens.reasoning,
                        cache_read: info.tokens.cache.read,
                        cache_write: info.tokens.cache.write,
                    });
                }
            }
            // Ignore other unhandled events
        }
    }

    Ok(())
}
