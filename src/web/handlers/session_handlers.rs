//! Session message, command, provider, and permission/question handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::api::interactions::CommandError;
use crate::api::ApiClient;
use crate::app::base_url;
use crate::runner::RunnerKind;
use crate::web::session_instructions;

/// Query parameters for paginated message fetching.
#[derive(serde::Deserialize)]
pub struct MessagePageQuery {
    /// Maximum number of messages to return. Omit or 0 for all.
    pub limit: Option<usize>,
    /// Only return messages created **before** this Unix-ms timestamp (exclusive).
    /// Used for "load older" pagination — pass the oldest timestamp from the
    /// previous page to fetch the preceding chunk.
    pub before: Option<u64>,
}

/// Selects which runner answers a directory-scoped catalog request (models, commands).
#[derive(serde::Deserialize, Default)]
pub struct RunnerQuery {
    /// Runtime runner whose catalog should be returned. Omitted means the default engine.
    pub runner: Option<String>,
}

/// GET /api/session/:id/messages — fetch messages for a session.
///
/// Supports optional pagination via query parameters:
///   - `?limit=N`             — return only the N most recent messages
///   - `?before=TIMESTAMP`    — return messages before this Unix-ms timestamp
///   - `?limit=N&before=T`    — load N messages before timestamp T
///
/// Response: `{ "messages": [...], "has_more": bool, "total": usize }`
/// Messages are sorted by creation time (ascending — oldest first within the page).
pub async fn get_session_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Query(page): Query<MessagePageQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
    {
        let body = state
            .runner_registry
            .messages(&session_id, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        return Ok(Json(paginate_messages(
            body,
            page.limit.unwrap_or(0),
            page.before,
        )));
    }
    let base = base_url().to_string();
    let resp = state
        .http_client
        .get(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    let limit = page.limit.unwrap_or(0);
    Ok(Json(paginate_messages(body, limit, page.before)))
}

/// Normalise a raw upstream messages payload into a flat, chronologically
/// sorted list and apply optional `limit` / `before` pagination.
///
/// Upstream may return either an array of messages or an object keyed by
/// message ID. Returns `{ messages, has_more, total }`.
pub(super) fn paginate_messages(
    body: serde_json::Value,
    limit: usize,
    before: Option<u64>,
) -> serde_json::Value {
    // Normalise the response into a flat Vec — upstream may return an array
    // or an object keyed by message ID.
    let mut all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    // Sort by info.time.created to ensure chronological order.
    all_messages.sort_by(|a, b| {
        let time_a = a
            .pointer("/info/time/created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let time_b = b
            .pointer("/info/time/created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        time_a.cmp(&time_b)
    });

    let total = all_messages.len();

    // Apply pagination: filter by `before` timestamp, then take last `limit`.
    if limit > 0 || before.is_some() {
        // Filter by `before` — keep only messages with created < before
        if let Some(before_ts) = before {
            all_messages.retain(|m| {
                let ts = m
                    .pointer("/info/time/created")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                ts < before_ts
            });
        }

        let filtered_count = all_messages.len();
        let effective_limit = if limit > 0 { limit } else { filtered_count };

        // Take only the last `limit` messages (most recent within the filtered set)
        let has_more = filtered_count > effective_limit;
        if has_more {
            all_messages = all_messages.split_off(filtered_count - effective_limit);
        }

        serde_json::json!({
            "messages": all_messages,
            "has_more": has_more,
            "total": total,
        })
    } else {
        // No pagination — return everything (backward compatible)
        serde_json::json!({
            "messages": all_messages,
            "has_more": false,
            "total": total,
        })
    }
}

/// Whether this send is the session's opening turn.
///
/// The in-memory latch answers instantly for every session this process has
/// already served. It cannot answer for a session that predates the process, so
/// that case asks the runner once whether the session holds any messages and
/// records the answer — one round-trip per old session, never per turn. An
/// engine that cannot answer is treated as a fresh session: a missed opening
/// block costs the user their standing instructions for a whole session, while
/// a duplicate costs one paragraph.
async fn first_turn(state: &ServerState, session_id: &str, dir: &str) -> bool {
    if state.web_state.instructions_delivered(session_id).await {
        return false;
    }
    let existing = state.runner_registry.messages(session_id, dir).await;
    let has_history = existing
        .ok()
        .and_then(|messages| messages.as_array().map(|list| !list.is_empty()))
        .unwrap_or(false);
    if has_history {
        state
            .web_state
            .mark_instructions_delivered(session_id)
            .await;
        return false;
    }
    true
}

/// POST /api/session/:id/message — send a message to a session.
pub async fn send_message(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(mut req): Json<SendMessageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // The session's own runner, when we know it. Recording the binding before
    // routing keeps an unnamed runner from being read as "switch to the default
    // runner", which would fork the session into a handoff.
    let owner = state
        .web_state
        .session_runner(&session_id)
        .await
        .and_then(|label| RunnerKind::parse(&label));
    if let Some(ref kind) = owner {
        state
            .runner_registry
            .ensure_binding(&session_id, kind.clone(), &dir)
            .await;
    }
    // `runner` is present only when the caller wants that runner specifically —
    // a new session's first turn, or a deliberate switch. A session with a known
    // owner goes through the registry regardless, so an omitted runner still
    // reaches the engine holding the transcript and gets that engine's own
    // effort/permission mapping. Only sessions opman knows nothing about fall
    // through to the raw upstream call.
    let route_through_registry = req.runner.is_some() || owner.is_some();

    // Session instructions open a session, they do not accompany every turn.
    // A requested runner that differs from the session's owner is about to mint
    // a handoff session, and that handoff message is the new session's first
    // turn — so it carries the block even when the session being left already
    // had it.
    let opens_handoff = req
        .runner
        .as_ref()
        .zip(owner.as_ref())
        .is_some_and(|(requested, current)| requested != current);
    let instructions_delivered = if first_turn(&state, &session_id, &dir).await || opens_handoff {
        let instructions = state
            .web_state
            .list_active_memory(
                Some(state.web_state.active_project_index().await),
                Some(&session_id),
            )
            .await;
        session_instructions::apply_to_parts(&mut req.parts, &instructions)
    } else {
        false
    };
    if instructions_delivered {
        state
            .web_state
            .mark_instructions_delivered(&session_id)
            .await;
    }

    // The session is running from here on. No runner reports a turn before it
    // has accepted it, and an interface that reads idle for that beat looks
    // like the prompt was dropped. The mark expires on its own, so a send that
    // never reaches a runner cannot strand the session as running.
    state.web_state.mark_turn_dispatched(&session_id).await;

    if route_through_registry {
        let requested = req.runner.clone();
        let request_body = serde_json::to_value(&req)
            .map_err(|e| WebError::Internal(format!("Invalid message: {e}")))?;
        let outcome = state
            .runner_registry
            .send_message(&session_id, &dir, requested, request_body)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        state
            .web_state
            .set_session_runner(&outcome.session_id, &outcome.runner.display_name())
            .await;

        // A handoff creates a runner-native session. Add it to the same
        // project immediately so the sidebar can render the new runner before
        // the normal session poller sees it.
        if outcome.switched {
            let project_idx = state.web_state.active_project_index().await;
            let now = chrono::Utc::now().timestamp_millis() as u64;
            state
                .web_state
                .add_and_activate_session(
                    project_idx,
                    crate::app::SessionInfo {
                        id: outcome.session_id.clone(),
                        title: "Handoff session".to_string(),
                        directory: dir.clone(),
                        time: crate::app::SessionTime {
                            created: now,
                            updated: now,
                        },
                        ..Default::default()
                    },
                )
                .await;
            state
                .web_state
                .set_session_runner(&outcome.session_id, &outcome.runner.display_name())
                .await;
            // The handoff prompt was this session's opening turn.
            state
                .web_state
                .mark_instructions_delivered(&outcome.session_id)
                .await;
            // The turn moved to the new session; the one being left is done.
            state.web_state.mark_turn_settled(&session_id).await;
            state
                .web_state
                .mark_turn_dispatched(&outcome.session_id)
                .await;
        }
        return Ok(Json(serde_json::json!({
            "ok": true,
            "session_id": outcome.session_id,
            "runner": outcome.runner,
            "switched": outcome.switched,
            "response": outcome.response,
        })));
    }

    let base = base_url().to_string();
    let resp = state
        .http_client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&req)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    map_send_message_response(&session_id, status, body)
}

/// Map a `send_message` upstream response into the handler result.
///
/// On success the raw body is relayed verbatim; on failure the upstream status
/// and body are logged and surfaced as an internal error.
pub(crate) fn map_send_message_response(
    session_id: &str,
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        tracing::error!(
            %session_id,
            upstream_status = %status,
            upstream_body = %body,
            "send_message: upstream rejected"
        );
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
}

/// POST /api/session/:id/abort — abort a running session.
pub async fn abort_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    // Drop the dispatch grace first: an abort that lands inside it must not be
    // outlived by the mark that was holding the session running.
    state.web_state.mark_turn_settled(&session_id).await;
    clear_asked_questions(&state, &session_id, &dir).await;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
    {
        state
            .runner_registry
            .abort(&session_id, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        return Ok(StatusCode::OK);
    }
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .abort_session(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// GET /api/session/:id/queue — list queued follow-up prompts.
pub async fn get_session_queue(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::GET, &session_id, None).await
}

/// DELETE /api/session/:id/queue — clear all queued follow-ups.
pub async fn clear_session_queue(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::DELETE, &session_id, None).await
}

/// DELETE /api/session/:id/queue/:index — remove one queued follow-up by index.
pub async fn remove_session_queue_item(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path((session_id, index)): axum::extract::Path<(String, usize)>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::DELETE, &session_id, Some(index)).await
}

/// Forward a queue request to the engine and relay its JSON body.
async fn proxy_queue(
    state: &ServerState,
    method: reqwest::Method,
    session_id: &str,
    index: Option<usize>,
) -> WebResult<Json<serde_json::Value>> {
    let dir = resolve_project_dir(state).await?;
    let base = base_url().to_string();
    let path = match index {
        Some(i) => format!("{}/session/{}/queue/{}", base, session_id, i),
        None => format!("{}/session/{}/queue", base, session_id),
    };
    let resp = state
        .http_client
        .request(method, path)
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    map_proxy_json_response(status, body)
}

/// Relay a proxied upstream JSON body, mapping non-2xx statuses to an internal
/// error carrying the upstream status and body. Shared by the queue and rename
/// proxies (identical semantics).
pub(crate) fn map_proxy_json_response(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
}

/// DELETE /api/session/:id — delete a session.
pub async fn delete_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
        && state
            .runner_registry
            .delete(&session_id, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?
    {
        return Ok(StatusCode::OK);
    }
    let base = base_url().to_string();
    let resp = state
        .http_client
        .delete(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = if status.is_success() {
        serde_json::Value::Null
    } else {
        resp.json().await.unwrap_or(serde_json::Value::Null)
    };
    map_status_only_response(status, body)
}

/// Map a status-only upstream response (delete): success → `200 OK`, otherwise
/// an internal error carrying the upstream status and body.
pub(crate) fn map_status_only_response(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<StatusCode> {
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(StatusCode::OK)
}

/// PATCH /api/session/:id — rename a session (update title).
pub async fn rename_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
        && state
            .runner_registry
            .rename(&session_id, &req.title, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?
    {
        return Ok(Json(serde_json::json!({ "ok": true, "title": req.title })));
    }
    let base = base_url().to_string();
    let resp = state
        .http_client
        .patch(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "title": req.title }))
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    map_proxy_json_response(status, body)
}

/// POST /api/session/:id/command — execute a slash command.
///
/// A slash command is a turn, so it routes the way a turn routes: to the runner that owns
/// the session. `GET /api/commands` already answers per runner, and offering a runner's own
/// vocabulary while executing it against the default engine is how a listed command comes
/// back "session not found". Only sessions opman has no binding for fall through to the
/// raw upstream call.
pub async fn execute_command(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<ExecuteCommandRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // Same binding-before-routing rule as `send_message`: recording the session's runner
    // first keeps a command from being answered by the default engine.
    if let Some(kind) = state
        .web_state
        .session_runner(&session_id)
        .await
        .and_then(|label| RunnerKind::parse(&label))
    {
        state
            .runner_registry
            .ensure_binding(&session_id, kind, &dir)
            .await;
    }
    if state.runner_registry.has_binding(&session_id).await {
        let result = state
            .runner_registry
            .execute_command(
                &session_id,
                &dir,
                crate::runner::SlashCommand::new(
                    &req.command,
                    &req.arguments,
                    req.model.as_deref(),
                ),
            )
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        // A command is a turn: mark it dispatched so the session reads as running for the
        // beat before the runner reports the turn it just accepted.
        state.web_state.mark_turn_dispatched(&session_id).await;
        return Ok(Json(result));
    }

    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let result = client
        .execute_session_command(
            &base,
            &dir,
            &session_id,
            &req.command,
            &req.arguments,
            req.model.as_deref(),
        )
        .await
        .map_err(|e| map_command_error(&e))?;
    Ok(Json(result))
}

/// Map an error from an upstream session command into a `WebError`.
///
/// A `CommandError` preserves the upstream HTTP status; anything else is logged
/// and collapsed into a generic internal error.
pub(crate) fn map_command_error(e: &anyhow::Error) -> WebError {
    if let Some(cmd_err) = e.downcast_ref::<CommandError>() {
        let status =
            StatusCode::from_u16(cmd_err.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        WebError::Upstream(status, cmd_err.message.clone())
    } else {
        tracing::error!("Session command failed: {e}");
        WebError::Internal("Command execution failed".into())
    }
}

/// GET /api/providers — fetch available providers and models.
pub async fn get_providers(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<RunnerQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if let Some(name) = query.runner.as_deref() {
        let runner = parse_runner(name)?;
        let providers = state
            .runner_registry
            .providers(runner, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        return Ok(Json(providers));
    }
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let providers = client
        .fetch_providers(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(providers))
}

/// GET /api/commands — the slash commands the runner advertises for this directory.
///
/// Answered by the runner named in `?runner=`, because a slash command is executed by the
/// agent and every agent has a different set: opencode's own command API, claude's
/// `slash_commands`, and whatever an ACP agent reports in `available_commands_update`.
/// Without a runner the request falls back to the default engine.
pub async fn get_commands(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<RunnerQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if let Some(name) = query.runner.as_deref() {
        let runner = parse_runner(name)?;
        let commands = state
            .runner_registry
            .commands(runner, &dir)
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        return Ok(Json(commands));
    }
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let cmds = client
        .list_commands(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(cmds))
}

/// Resolve a runner name from a query string, or reject the request.
fn parse_runner(name: &str) -> WebResult<crate::runner::RunnerKind> {
    crate::runner::RunnerKind::parse(name)
        .ok_or_else(|| WebError::BadRequest(format!("Unknown runner: {name}")))
}

/// POST /api/permission/:id/reply — reply to a permission request.
pub async fn reply_permission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<PermissionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .reply_permission(&request_id, &req.reply)
        .await
        .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?
    {
        return Ok(StatusCode::OK);
    }
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_permission(&base, &dir, &request_id, &req.reply)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// POST /api/question/:id/reply — reply to a question.
///
/// The local registry is asked first: a question raised through the `ask` MCP server is
/// held by this process, and fanning it out to the engines would find no owner and then
/// fail against the upstream server.
pub async fn reply_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<QuestionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let Err(answers) = state.ask_pending.resolve(&request_id, req.answers) else {
        return Ok(StatusCode::OK);
    };
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .reply_question(&request_id, &answers)
        .await
        .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?
    {
        return Ok(StatusCode::OK);
    }
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_question(&base, &dir, &request_id, &answers)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// Take down any question the `ask` server raised for a session that is being aborted.
///
/// The engines already do this for their own prompts; a locally-held question has to be
/// cleared here because nothing else knows about it. Leaving one up would invite the user
/// to answer into a turn that has already unwound — and the asker is being killed with the
/// turn, so nothing would receive the answer.
async fn clear_asked_questions(state: &ServerState, session_id: &str, dir: &str) {
    for id in state.ask_pending.clear_session(session_id) {
        state
            .web_state
            .publish_event(
                &serde_json::json!({
                    "type": "question.rejected",
                    "properties": { "id": id, "requestID": id, "sessionID": session_id },
                }),
                dir,
            )
            .await;
    }
}

/// POST /api/question/:id/reject — dismiss a question without answering it.
///
/// The frontend has always called this; until now nothing served it, so dismissing a card
/// left the asker blocked. Rejecting is best-effort by design: a card the user dismisses
/// after its asker already gave up still has to disappear, so an id nobody owns is a
/// success, not a 404.
pub async fn reject_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    if state.ask_pending.dismiss(&request_id) {
        return Ok(StatusCode::OK);
    }
    state
        .runner_registry
        .reject_question(&request_id)
        .await
        .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
    Ok(StatusCode::OK)
}

/// GET /api/pending — return pending permissions and questions across all sessions.
pub async fn get_pending(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let ws = state.web_state.inner.read().await;
    let permissions: Vec<&serde_json::Value> = ws.pending_permissions.values().collect();
    let questions: Vec<&serde_json::Value> = ws.pending_questions.values().collect();
    Ok(Json(serde_json::json!({
        "permissions": permissions,
        "questions": questions,
    })))
}

/// POST /api/session/:id/mark_seen — clear unseen state for a session.
pub async fn mark_session_seen(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    state.web_state.mark_session_seen(&session_id).await;
    Ok(StatusCode::OK)
}

/// POST /api/session/:id/a2ui/callback — inject an A2UI interaction
/// (button click or form submission) back into the session as a user
/// message so the agent can see and react to it.
pub async fn a2ui_callback(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<A2uiCallbackRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // Format the callback as a structured user message the agent can parse.
    let text = a2ui_callback_text(&req.callback_id, &req.payload);

    let msg_body = serde_json::json!({
        "parts": [{
            "type": "text",
            "text": text,
        }]
    });

    let resp = state
        .http_client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&msg_body)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = if status.is_success() {
        serde_json::Value::Null
    } else {
        resp.json().await.unwrap_or(serde_json::Value::Null)
    };
    a2ui_callback_result(status, body)
}

/// Build the structured user-message text that represents an A2UI callback.
///
/// A null or empty-object payload produces a bare marker line; any other
/// payload is appended as a pretty-printed fenced JSON block.
pub(crate) fn a2ui_callback_text(callback_id: &str, payload: &serde_json::Value) -> String {
    if payload.is_null() || *payload == serde_json::json!({}) {
        format!("[A2UI callback: {}]", callback_id)
    } else {
        let payload_str = serde_json::to_string_pretty(payload).unwrap_or_default();
        format!(
            "[A2UI callback: {}]\n```json\n{}\n```",
            callback_id, payload_str
        )
    }
}

/// Map the A2UI callback upstream response: success → `{ "ok": true }`,
/// otherwise an internal error carrying the upstream status and body.
pub(crate) fn a2ui_callback_result(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
#[path = "session_handlers_direct_tests.rs"]
mod session_handlers_direct_tests;

#[cfg(test)]
#[path = "session_handlers_proxy_tests.rs"]
mod session_handlers_proxy_tests;

#[cfg(test)]
#[path = "session_handlers_maps_tests.rs"]
mod session_handlers_maps_tests;

#[cfg(test)]
#[path = "session_handlers_upstream_tests.rs"]
mod session_handlers_upstream_tests;

#[cfg(test)]
#[path = "session_handlers_commands_tests.rs"]
mod session_handlers_commands_tests;

#[cfg(test)]
#[path = "session_handlers_question_tests.rs"]
mod session_handlers_question_tests;
