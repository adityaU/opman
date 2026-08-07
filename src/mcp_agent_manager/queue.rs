//! Delivering a message now, or on the target's next idle turn.

use anyhow::Result;
use serde_json::{json, Value};

use super::ManagerState;
use crate::runner::RunnerKind;

/// How often the worker looks for a target that has gone idle.
const POLL: std::time::Duration = std::time::Duration::from_millis(250);

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
    let outcome = state
        .registry
        .send_message(
            &message.target,
            &message.directory,
            message.runner,
            message.body,
        )
        .await?;
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
            if state
                .registry
                .send_message(
                    &message.target,
                    &message.directory,
                    message.runner.clone(),
                    message.body.clone(),
                )
                .await
                .is_err()
            {
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
