//! The server-side half of "switch on its own": a background reading of every account,
//! the redirect applied when a message is sent to a low runner, and the retry after a
//! turn that failed on a limit.
//!
//! Everything here runs without a browser. A session moved this way is remembered as a
//! *forward* from its old id to its new one, because the callers that drive background
//! work (kanban, the agent-manager, routines) hold on to the id they started with — and
//! a send to that id must reach the conversation where it continues, not fork it again.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::Duration;

use chrono::Utc;
use serde_json::{json, Value};

use crate::runner::{RunnerKind, RunnerRegistry};
use crate::usage::types::Account;

use super::choose::{choose, runner_level, Accounts, Level};
use super::settings::{self, FallbackPair, LimitPolicy};

/// How often the accounts are re-read. The usage service caches for the same minute, so a
/// browser asking in between costs nothing extra.
const POLL: Duration = Duration::from_secs(60);
/// Failed turns remembered as already handled. Old ones only matter for a minute or two.
const HANDLED_CAP: usize = 256;

/// What the turn that ran out is told on the runner that takes over. The handoff prompt
/// already carries the whole transcript, the failed request included.
const CONTINUE_PROMPT: &str = "The previous runner ran out of usage before it finished. \
Continue the last request from where it stopped.";

/// A session that moved to another runner.
#[derive(Clone, Debug)]
pub struct Handoff {
    pub from: String,
    pub to: String,
    pub runner: RunnerKind,
}

#[derive(Default)]
pub struct LimitWatch {
    accounts: RwLock<Arc<Accounts>>,
    forwards: Mutex<HashMap<String, String>>,
    handled: Mutex<VecDeque<String>>,
}

impl LimitWatch {
    pub fn accounts(&self) -> Arc<Accounts> {
        let guard = self
            .accounts
            .read()
            .unwrap_or_else(|poison| poison.into_inner());
        Arc::clone(&guard)
    }

    pub fn store(&self, accounts: Vec<Account>) {
        let next: Accounts = accounts
            .into_iter()
            .map(|account| (account.id, account))
            .collect();
        let mut guard = self
            .accounts
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        *guard = Arc::new(next);
    }

    /// Where a session moved to, following a chain of moves to its end.
    pub fn forward_of(&self, session_id: &str) -> Option<String> {
        let forwards = self
            .forwards
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut current = forwards.get(session_id)?;
        // Bounded by the table size, so a cycle (which `record_forward` never creates)
        // could not spin forever.
        for _ in 0..forwards.len() {
            match forwards.get(current) {
                Some(next) => current = next,
                None => break,
            }
        }
        Some(current.clone())
    }

    pub fn record_forward(&self, from: &str, to: &str) {
        if from == to {
            return;
        }
        let mut forwards = self
            .forwards
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        forwards.insert(from.to_string(), to.to_string());
    }

    /// True the first time an id is seen, so one failure is acted on once.
    fn first_time(&self, id: &str) -> bool {
        let mut handled = self
            .handled
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if handled.iter().any(|seen| seen == id) {
            return false;
        }
        if handled.len() >= HANDLED_CAP {
            handled.pop_front();
        }
        handled.push_back(id.to_string());
        true
    }
}

pub fn global() -> &'static LimitWatch {
    static WATCH: LazyLock<LimitWatch> = LazyLock::new(LimitWatch::default);
    &WATCH
}

/// Re-read every account each minute while the policy is on, for as long as opman runs.
pub fn spawn_poller(client: reqwest::Client) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(POLL);
        loop {
            tick.tick().await;
            if settings::global().get().policy == LimitPolicy::Off {
                continue;
            }
            let accounts = crate::usage::global().accounts(&client, false).await;
            global().store(accounts);
        }
    });
}

/// Point a send body at `pair`. Agent and effort name things in the old runner's
/// catalogue, so they are dropped rather than carried across. A configured permission
/// replaces the caller's choice; the handoff checks it against the target catalogue.
fn retarget(body: &mut Value, pair: &FallbackPair) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    object.remove("agent");
    object.remove("effort");
    if let Some(permission) = &pair.permission {
        object.insert("permission".into(), Value::String(permission.clone()));
    }
    match &pair.model {
        Some(model) => {
            object.insert(
                "model".into(),
                json!({ "providerID": model.provider_id, "modelID": model.model_id }),
            );
        }
        None => {
            object.remove("model");
        }
    }
}

/// The runner a message to a low runner should go to instead, with `body` rewritten for
/// it. `None` leaves the send alone: the policy is not "auto", the runner has room, or no
/// pair of the user's list would get further.
pub fn redirect(
    current: &RunnerKind,
    available: &[RunnerKind],
    body: &mut Value,
) -> Option<RunnerKind> {
    let settings = settings::global().get();
    if settings.policy != LimitPolicy::Auto {
        return None;
    }
    let accounts = global().accounts();
    let now = Utc::now();
    let level = runner_level(current, &accounts, settings.near_percent, now)?;
    let (kind, pair) = choose(&settings, current, level, available, &accounts, now)?;
    retarget(body, pair);
    Some(kind)
}

/// A turn failed on a limit: move the session to the first pair with room and continue.
/// Returns the handoff it made, or `None` when there was nothing to do.
pub async fn on_limit_hit(
    registry: &RunnerRegistry,
    session_id: &str,
    directory: &str,
    message_id: &str,
) -> Option<Handoff> {
    let settings = settings::global().get();
    if settings.policy != LimitPolicy::Auto || !global().first_time(message_id) {
        return None;
    }
    let current = registry.runner_for(session_id).await;
    let accounts = global().accounts();
    let (kind, pair) = choose(
        &settings,
        &current,
        Level::Out,
        &registry.available(),
        &accounts,
        Utc::now(),
    )?;
    let mut body = json!({ "parts": [{ "type": "text", "text": CONTINUE_PROMPT }] });
    retarget(&mut body, pair);
    match registry
        .send_message(session_id, directory, Some(kind), body)
        .await
    {
        Ok(outcome) if outcome.switched => {
            global().record_forward(session_id, &outcome.session_id);
            Some(Handoff {
                from: session_id.to_string(),
                to: outcome.session_id,
                runner: outcome.runner,
            })
        }
        Ok(_) => None,
        Err(error) => {
            tracing::warn!("runner limit: could not move {session_id}: {error:#}");
            None
        }
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod watch_tests;

#[cfg(test)]
#[path = "handoff_tests.rs"]
mod handoff_tests;
