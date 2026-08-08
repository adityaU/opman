//! Delivering a message now, or on the target's next idle turn.

use anyhow::Result;
use serde_json::{json, Value};

use super::ManagerState;
use crate::runner::RunnerKind;

/// How often the worker looks for a target that has gone idle.
const POLL: std::time::Duration = std::time::Duration::from_millis(250);

/// How long a dispatch may take before it is reported as stuck.
///
/// Handing a message to a runner is meant to be quick — every engine acknowledges the
/// dispatch and runs the turn behind it — so a send that has not returned by now is a
/// wedged runner, not a slow model. Without this bound the caller's *tool call* hangs
/// instead, which is the worst possible failure: the calling agent sits inside a tool that
/// will never return, and cannot be told why.
const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Clone, Debug)]
pub(super) struct QueuedMessage {
    pub(super) id: String,
    pub(super) source: String,
    pub(super) target: String,
    pub(super) directory: String,
    pub(super) runner: Option<RunnerKind>,
    pub(super) body: Value,
}

/// Steer the target now, or wait for it to finish what it is doing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Delivery {
    Immediate,
    Queued,
}

impl Delivery {
    pub(super) fn parse(value: Option<&str>) -> Result<Option<Self>> {
        match value
            .unwrap_or("immediate")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "immediate" | "steer" => Ok(Some(Self::Immediate)),
            "queued" | "next_turn" | "next-turn" => Ok(Some(Self::Queued)),
            other => anyhow::bail!("delivery must be 'immediate' or 'queued', got '{other}'"),
        }
    }
}

pub(super) async fn deliver(
    state: &ManagerState,
    message: QueuedMessage,
    delivery: Option<Delivery>,
) -> Result<Value> {
    if delivery == Some(Delivery::Queued) {
        let id = message.id.clone();
        state
            .queues
            .lock()
            .await
            .entry(message.target.clone())
            .or_default()
            .push(message);
        return Ok(json!({ "message_id": id, "delivery": "queued" }));
    }
    let outcome = tokio::time::timeout(
        SEND_TIMEOUT,
        state.registry.send_message(
            &message.target,
            &message.directory,
            message.runner,
            message.body,
        ),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "'{}' did not accept the message within {}s. Its runner may be restarting; \
             call agent_progress to check, then retry.",
            message.target,
            SEND_TIMEOUT.as_secs()
        )
    })??;
    if let Some(error) = runner_error(&outcome.response) {
        anyhow::bail!("the target refused the message: {error}");
    }
    // A runner switch forks a new session; the parent link has to follow it, or the child
    // loses the agent it was told to answer.
    if outcome.switched && !message.source.is_empty() {
        state
            .parents
            .lock()
            .await
            .insert(outcome.session_id.clone(), message.source.clone());
    }
    Ok(json!({
        "message_id": message.id,
        "delivery": "immediate",
        "session_id": outcome.session_id,
        "runner": outcome.runner,
        "switched": outcome.switched,
        "response": outcome.response,
    }))
}

/// The runner's own refusal, if the reply carries one.
///
/// A dispatch that a runner rejected still comes back as `Ok` with the rejection inside
/// it, and reporting that to the caller as a delivered message is a lie it cannot detect:
/// the tool says "immediate", the turn never happened, and the only way to find out was to
/// poll the transcript and read the raw error.
fn runner_error(response: &Value) -> Option<String> {
    if response.get("ok").and_then(Value::as_bool) == Some(false) {
        return Some(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("the runner reported a failure")
                .to_string(),
        );
    }
    let error = response.get("error")?;
    let message = error
        .get("message")
        .or_else(|| error.pointer("/data/message"))
        .and_then(Value::as_str)
        .or_else(|| error.as_str())?;
    Some(message.to_string())
}

/// Drain queued messages as their targets go idle.
///
/// A send that fails goes back to the front of its queue rather than being dropped: the
/// usual cause is the target restarting, and the message is still wanted once it is back.
pub(super) async fn worker(state: ManagerState) {
    let mut interval = tokio::time::interval(POLL);
    loop {
        interval.tick().await;
        let targets: Vec<String> = state.queues.lock().await.keys().cloned().collect();
        for target in targets {
            let Some(first) = state
                .queues
                .lock()
                .await
                .get(&target)
                .and_then(|queue| queue.first())
                .cloned()
            else {
                continue;
            };
            let Ok(progress) = state.registry.progress(&target, &first.directory).await else {
                continue;
            };
            if progress.get("busy").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let message = {
                let mut queues = state.queues.lock().await;
                queues
                    .get_mut(&target)
                    .filter(|queue| !queue.is_empty())
                    .map(|queue| queue.remove(0))
            };
            let Some(message) = message else { continue };
            let sent = tokio::time::timeout(
                SEND_TIMEOUT,
                state.registry.send_message(
                    &message.target,
                    &message.directory,
                    message.runner.clone(),
                    message.body.clone(),
                ),
            )
            .await;
            if !matches!(sent, Ok(Ok(_))) {
                state
                    .queues
                    .lock()
                    .await
                    .entry(target)
                    .or_default()
                    .insert(0, message);
            }
        }
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod queue_tests;

#[cfg(test)]
#[path = "queue_delivery_tests.rs"]
mod queue_delivery_tests;
