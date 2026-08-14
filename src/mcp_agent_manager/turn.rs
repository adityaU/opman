//! The two operations that start a turn: steering an agent, and creating one to steer.
//!
//! Together because they answer the same three questions — which agent, running as what,
//! and under whose permission — and differ only in whether the session exists yet.
//!
//! The permission is the part that had no answer at all. A session created here took
//! whatever mode its agent happened to open in (codex `agent`, claude "Manual"), which is
//! the one setting an unattended fleet cannot live with: every tool call stops on a
//! prompt that the agent who started the session has no way to answer.

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::runner::RunnerKind;

use super::permission::PermissionMode;
use super::queue::{self, Delivery, QueuedMessage};
use super::request::ManagerRequest;
use super::{resolve_target, ManagerState};

/// Hand a message to an agent that already exists.
pub(super) async fn send(
    state: &ManagerState,
    request: &ManagerRequest,
    directory: &str,
    source: String,
    runner: Option<RunnerKind>,
    delivery: Option<Delivery>,
) -> Result<Value> {
    let target = resolve_target(state, request.target.as_deref(), &source).await?;
    let message = request
        .message
        .as_deref()
        .context("agent_send requires 'message'")?;
    // The target's own runner, unless this send is the switch that changes it.
    let kind = match runner.clone() {
        Some(kind) => kind,
        None => state.registry.runner_for(&target).await,
    };
    let permission = requested(state, request, kind, directory).await?;
    let body = request
        .dispatch()?
        .with_permission(permission)
        .body(message);
    queue::deliver(
        state,
        QueuedMessage {
            id: new_id("msg"),
            source,
            target,
            directory: directory.to_string(),
            runner,
            body,
        },
        delivery,
    )
    .await
}

/// Create a session, remember who asked for it, and send the opening message.
///
/// Everything the session runs as is settled *before* it exists: a rejected model or an
/// unrecognised permission mode would otherwise leave an empty session behind that nobody
/// asked for, configured as the thing the caller was trying to avoid.
pub(super) async fn start(
    state: &ManagerState,
    request: &ManagerRequest,
    directory: &str,
    source: &str,
    runner: Option<RunnerKind>,
    delivery: Option<Delivery>,
) -> Result<Value> {
    let kind = match runner {
        Some(kind) => kind,
        None if !source.is_empty() => state.registry.runner_for(source).await,
        None => state.registry.default_kind(),
    };
    let dispatch = request
        .dispatch()?
        .with_permission(permission_for(state, request, &kind, directory, source).await?);
    let session = state
        .registry
        .create_session(
            kind.clone(),
            directory,
            request.title.as_deref().unwrap_or("Agent session"),
        )
        .await?;
    record(state, &session.id, directory, &dispatch).await;
    if !source.is_empty() {
        state
            .parents
            .lock()
            .await
            .insert(session.id.clone(), source.to_string());
    }
    let Some(message) = request
        .message
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return Ok(json!({
            "session_id": session.id,
            "runner": kind,
            "permission": dispatch.permission(),
            "delivery": "none",
            "parent_agent_id": (!source.is_empty()).then_some(source),
        }));
    };
    let message = opening_message(source, message);
    let mut result = queue::deliver(
        state,
        QueuedMessage {
            id: new_id("msg"),
            source: source.to_string(),
            target: session.id.clone(),
            directory: directory.to_string(),
            runner: None,
            body: dispatch.body(&message),
        },
        Some(delivery.unwrap_or(Delivery::Immediate)),
    )
    .await?;
    result["session_id"] = Value::String(session.id);
    result["runner"] = serde_json::to_value(kind)?;
    result["permission"] = json!(dispatch.permission());
    result["parent_agent_id"] = (!source.is_empty()).then_some(source).into();
    Ok(result)
}

/// Tell a child who launched it and how to return its final result.
///
/// The parent link is also kept in manager state, but that is invisible to the child. The
/// opening prompt is the one reliable place to pass the identity across the runner
/// boundary, including when the opening turn is queued.
fn opening_message(source: &str, message: &str) -> String {
    if source.is_empty() {
        return message.to_string();
    }
    format!(
        "{message}\n\n[Agent handoff]\nYou were started by agent `{source}`. When your work is complete, report the final work back to that agent with `agent_send`: set `to` to `{source}`, include a concise summary of what you changed, files touched, tests or checks run, and any blockers. Before sending, call `agent_runner_options` if you need to choose the required `model` and `effort` values. Do not finish without sending that report."
    )
}

/// Write the choices onto the session before anything runs on it.
///
/// Best-effort by design: a runner with no configure route answers `false`, and the send
/// path carries the same values on the turn itself. What this adds is the case with no
/// turn to carry them — a session started without an opening message, whose agent would
/// otherwise be spawned in its own default mode the first time anyone prompted it.
async fn record(state: &ManagerState, session: &str, directory: &str, dispatch: &super::Dispatch) {
    if let Err(error) = state
        .registry
        .configure(session, directory, &dispatch.choices())
        .await
    {
        tracing::debug!(%session, %error, "new session kept its runner's own configuration");
    }
}

/// The mode a new session runs under: the one asked for, else the caller's own.
async fn permission_for(
    state: &ManagerState,
    request: &ManagerRequest,
    kind: &RunnerKind,
    directory: &str,
    source: &str,
) -> Result<Option<PermissionMode>> {
    let Some(mode) = requested(state, request, kind.clone(), directory).await? else {
        return Ok(PermissionMode::inherited(&state.registry, kind, directory, source).await);
    };
    Ok(Some(mode))
}

/// The mode the caller named, checked against the runner that will run it.
async fn requested(
    state: &ManagerState,
    request: &ManagerRequest,
    kind: RunnerKind,
    directory: &str,
) -> Result<Option<PermissionMode>> {
    let Some(requested) = request.requested_permission() else {
        return Ok(None);
    };
    PermissionMode::resolve(&state.registry, kind, directory, requested)
        .await
        .map(Some)
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", rand::random::<u128>())
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod turn_tests;
