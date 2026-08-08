//! Common runner abstraction used by the web backend.
//!
//! A session is deliberately kept separate from a runner.  This is what lets a
//! client select a different runner for the next turn: the registry creates a
//! new runner-native session, sends a handoff context to it, and returns the
//! new session id to the client.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, RwLock};

pub type RunnerFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub use opman_backend_contracts::{
    is_valid_acp_id, register_acp_runners, ProjectDirectory, RunnerKind, SessionId,
};

/// The small contract every runner implements.  The web layer only deals in
/// opencode-shaped JSON, so runner-specific protocol details stay here.
pub trait Runner: Send + Sync {
    fn kind(&self) -> RunnerKind;
    fn event_url(&self) -> Option<String> {
        None
    }
    fn event_receiver(&self) -> Option<broadcast::Receiver<String>> {
        None
    }
    fn create_session<'a>(
        &'a self,
        directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession>;
    fn sessions<'a>(
        &'a self,
        _directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value>;
    /// Return the runner's current session status map.  Implementations use
    /// the same shape as OpenCode's `/session/status`: `{ session_id: { type:
    /// "busy" } }`; idle sessions may be omitted.
    fn status<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }
    fn providers<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({ "all": [], "connected": [], "default": {} })) })
    }
    /// The runner's own agent list. Must be asked of the runner rather than the default
    /// engine: every runner has its own idea of what an agent is, and proxying to whichever
    /// engine happens to be primary hands back another runner's agents.
    fn agents<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!([])) })
    }
    /// The runner's own slash commands, in the opencode `[{ name, description }]` shape.
    ///
    /// Same rule as `agents`: a slash command is executed by the runner, so only the runner
    /// can say which ones exist. A runner that advertises none simply has none — opman does
    /// not keep a table to fill the gap, because a table is wrong for every runner it was
    /// not written against.
    fn commands<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!([])) })
    }
    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value>;
    fn abort<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, ()>;
    fn rename<'a>(&'a self, _session_id: &'a str, _title: &'a str) -> RunnerFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn delete<'a>(&'a self, _session_id: &'a str) -> RunnerFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn reply_permission<'a>(
        &'a self,
        _request_id: &'a str,
        _reply: &'a str,
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn reply_question<'a>(
        &'a self,
        _request_id: &'a str,
        _answers: &'a [Vec<String>],
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
}

/// Whether one `/session/status` entry describes a turn that is still going.
///
/// Runners agree on the envelope but not on the word: opencode says `busy` or
/// `retry` while ACP agents say `active`. A retry is still the same unfinished
/// turn, so it counts as running. An entry without a recognised type is idle —
/// the map only lists non-idle sessions anyway.
pub fn is_running_status(entry: &Value) -> bool {
    entry
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "busy" | "retry" | "active"))
}

/// The session ids a `/session/status` map reports as running.
pub fn running_session_ids(status: &Value) -> HashSet<String> {
    let Some(entries) = status.as_object() else {
        return HashSet::new();
    };
    entries
        .iter()
        .filter(|(_, entry)| is_running_status(entry))
        .map(|(id, _)| id.clone())
        .collect()
}

/// A session a runner just allocated. The directory is not carried back: the caller
/// already holds the one it asked for, and the registry stores that on the binding.
#[derive(Clone, Debug)]
pub struct RunnerSession {
    pub id: String,
    pub title: String,
}

/// Adapter for runners exposing the opencode REST contract.  Both the native
/// opencode server and the embedded Claude adapter use this implementation.
pub struct HttpRunner {
    kind: RunnerKind,
    base_url: String,
    client: reqwest::Client,
}

impl HttpRunner {
    pub fn new(kind: RunnerKind, base_url: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            kind,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        }
    }

    /// POST a permission/question reply and report whether this engine owned the
    /// request. Transport errors, non-2xx statuses, and bodies without `"ok": true`
    /// all read as "not ours" — never as a hard failure — so the registry can try
    /// the other runners and the caller can still fall back to the default backend.
    async fn try_reply(&self, url: &str, body: Value) -> Result<bool> {
        let resp = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await;
        let Ok(resp) = resp else { return Ok(false) };
        if !resp.status().is_success() {
            return Ok(false);
        }
        let body = resp.json::<Value>().await.unwrap_or(Value::Null);
        Ok(body.get("ok").and_then(Value::as_bool).unwrap_or(false))
    }

    async fn json_request(&self, request: reqwest::RequestBuilder) -> Result<Value> {
        let response = request
            .header("Accept", "application/json")
            .send()
            .await
            .context("runner request failed")?;
        let status = response.status();
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!(
                "{} runner returned {}: {:?}",
                self.kind.display_name(),
                status,
                body
            );
        }
        Ok(body)
    }
}

impl Runner for HttpRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }
    fn event_url(&self) -> Option<String> {
        Some(format!("{}/event", self.base_url))
    }

    fn create_session<'a>(
        &'a self,
        directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            let body = self
                .json_request(
                    self.client
                        .post(format!("{}/session", self.base_url))
                        .header("x-opencode-directory", directory)
                        .json(&json!({ "title": title })),
                )
                .await?;
            let id = body
                .get("id")
                .and_then(Value::as_str)
                .context("runner did not return a session id")?;
            Ok(RunnerSession {
                id: id.to_string(),
                title: body
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(title)
                    .to_string(),
            })
        })
    }

    fn sessions<'a>(
        &'a self,
        directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        Box::pin(async move {
            let body = self
                .json_request(
                    self.client
                        .get(format!("{}/session", self.base_url))
                        .header("x-opencode-directory", directory),
                )
                .await?;
            Ok(serde_json::from_value(body).context("runner returned an invalid session list")?)
        })
    }

    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.json_request(
                self.client
                    .get(format!("{}/session/{}/message", self.base_url, session_id))
                    .header("x-opencode-directory", directory),
            )
            .await
        })
    }

    fn status<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.json_request(
                self.client
                    .get(format!("{}/session/status", self.base_url))
                    .header("x-opencode-directory", directory),
            )
            .await
        })
    }

    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let mut body = body;
            if self.kind == RunnerKind::Opencode {
                // OpenCode's message schema does not accept runner controls.
                if let Some(object) = body.as_object_mut() {
                    if let Some(effort) = object.remove("effort") {
                        object.insert("variant".to_string(), effort);
                    }
                    object.remove("permission");
                    object.remove("runner");
                    if matches!(
                        object.get("agent").and_then(Value::as_str),
                        Some("default") | Some("claude")
                    ) {
                        object.remove("agent");
                    }
                }
            }
            // `prompt_async`, never `/message`. Every HTTP runner exposes both,
            // but `POST /session/{id}/message` streams the turn and only
            // responds once the assistant is finished — OpenCode documents it
            // as "streaming the AI response" and returns the completed
            // AssistantMessage. Awaiting that holds the browser's POST open for
            // the whole turn, so any turn outliving the client or tunnel
            // timeout (~100s through Cloudflare) looks like a hang and fails,
            // even though the turn is running fine. `prompt_async` starts the
            // turn and returns immediately; the transcript arrives over SSE,
            // which is how the UI renders replies anyway.
            self.json_request(
                self.client
                    .post(format!(
                        "{}/session/{}/prompt_async",
                        self.base_url, session_id
                    ))
                    .header("x-opencode-directory", directory)
                    .json(&body),
            )
            .await
        })
    }

    fn providers<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let body = self
                .json_request(
                    self.client
                        .get(format!("{}/provider", self.base_url))
                        .header("x-opencode-directory", directory),
                )
                .await?;
            Ok(body)
        })
    }

    fn agents<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let body = self
                .json_request(
                    self.client
                        .get(format!("{}/agent", self.base_url))
                        .header("x-opencode-directory", directory),
                )
                .await?;
            Ok(body)
        })
    }

    fn commands<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let body = self
                .json_request(
                    self.client
                        .get(format!("{}/command", self.base_url))
                        .header("x-opencode-directory", directory),
                )
                .await?;
            Ok(body)
        })
    }

    fn abort<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async move {
            self.json_request(
                self.client
                    .post(format!("{}/session/{}/abort", self.base_url, session_id))
                    .header("x-opencode-directory", directory),
            )
            .await
            .map(|_| ())
        })
    }

    // Permission/question replies are routed by request id, and only the engine
    // that raised the request knows the id. Each engine's reply endpoint reports
    // whether it actually resolved a pending waiter (`{"ok": bool}`), so an engine
    // that doesn't own the id — or doesn't implement the route — reads as "not
    // ours" and the registry keeps looking instead of stopping at the first 200.
    fn reply_permission<'a>(
        &'a self,
        request_id: &'a str,
        reply: &'a str,
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            self.try_reply(
                &format!("{}/permission/{}/reply", self.base_url, request_id),
                json!({ "reply": reply }),
            )
            .await
        })
    }

    fn reply_question<'a>(
        &'a self,
        request_id: &'a str,
        answers: &'a [Vec<String>],
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            self.try_reply(
                &format!("{}/question/{}/reply", self.base_url, request_id),
                json!({ "answers": answers }),
            )
            .await
        })
    }
}

/// Runner wrapper for an embedded ACP engine that exposes an in-process broadcast
/// receiver instead of an HTTP SSE URL. This bypasses the HTTP relay, which batches
/// rapid streaming events into a single TCP chunk that the frontend's debounce then
/// collapses into one render — the "all at once" effect that per-token ACP streaming
/// exists to avoid.
pub struct AcpRunner {
    http: HttpRunner,
    engine: Arc<crate::acp_engine::AcpEngine>,
}

impl AcpRunner {
    pub fn new(
        kind: RunnerKind,
        url: impl Into<String>,
        client: reqwest::Client,
        engine: Arc<crate::acp_engine::AcpEngine>,
    ) -> Self {
        Self {
            http: HttpRunner::new(kind, url, client),
            engine,
        }
    }
}

impl Runner for AcpRunner {
    fn kind(&self) -> RunnerKind {
        self.http.kind()
    }
    fn event_url(&self) -> Option<String> {
        None
    }
    fn event_receiver(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.engine.subscribe_raw())
    }
    fn create_session<'a>(
        &'a self,
        directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        self.http.create_session(directory, title)
    }
    fn sessions<'a>(
        &'a self,
        directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        self.http.sessions(directory)
    }
    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value> {
        self.http.messages(session_id, directory)
    }
    fn status<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        self.http.status(directory)
    }
    fn providers<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        self.http.providers(directory)
    }
    fn agents<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        self.http.agents(directory)
    }
    fn commands<'a>(&'a self, directory: &'a str) -> RunnerFuture<'a, Value> {
        self.http.commands(directory)
    }
    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        self.http.send_message(session_id, directory, body)
    }
    fn abort<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, ()> {
        self.http.abort(session_id, directory)
    }
    fn rename<'a>(&'a self, session_id: &'a str, title: &'a str) -> RunnerFuture<'a, bool> {
        self.http.rename(session_id, title)
    }
    fn delete<'a>(&'a self, session_id: &'a str) -> RunnerFuture<'a, bool> {
        self.http.delete(session_id)
    }
    fn reply_permission<'a>(
        &'a self,
        request_id: &'a str,
        reply: &'a str,
    ) -> RunnerFuture<'a, bool> {
        self.http.reply_permission(request_id, reply)
    }
    fn reply_question<'a>(
        &'a self,
        request_id: &'a str,
        answers: &'a [Vec<String>],
    ) -> RunnerFuture<'a, bool> {
        self.http.reply_question(request_id, answers)
    }
}

#[derive(Clone, Debug)]
struct Binding {
    runner: RunnerKind,
    physical_id: String,
    directory: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SendOutcome {
    pub session_id: String,
    pub runner: RunnerKind,
    pub switched: bool,
    pub response: Value,
}

/// The runners on offer right now. Held behind one pointer so a read is an `Arc` clone
/// under a short lock, never a map clone, and never a lock held across an await.
type RunnerMap = Arc<HashMap<RunnerKind, Arc<dyn Runner>>>;

/// Routes logical sessions to runner-native sessions and performs handoffs.
///
/// The runner set is not fixed for the life of the process: ACP agents are declared in
/// config, so adding, editing or removing one from the settings page installs or drops a
/// runner underneath sessions already in flight. Writes copy the map and swap the pointer,
/// which is the right trade when reads happen on every request and writes on a config edit.
pub struct RunnerRegistry {
    default: RunnerKind,
    runners: std::sync::RwLock<RunnerMap>,
    bindings: RwLock<HashMap<String, Binding>>,
}

impl RunnerRegistry {
    pub fn new(default: RunnerKind, runners: HashMap<RunnerKind, Arc<dyn Runner>>) -> Self {
        Self {
            default,
            runners: std::sync::RwLock::new(Arc::new(runners)),
            bindings: RwLock::new(HashMap::new()),
        }
    }

    /// The runner set as of this moment.
    ///
    /// A poisoned lock returns the value anyway: a panic in an unrelated reader must not
    /// leave every session unroutable.
    fn snapshot(&self) -> RunnerMap {
        match self.runners.read() {
            Ok(guard) => Arc::clone(&guard),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    /// Replace the runner set with the result of `edit`, applied to a copy.
    fn mutate(&self, edit: impl FnOnce(&mut HashMap<RunnerKind, Arc<dyn Runner>>)) {
        let mut next = (*self.snapshot()).clone();
        edit(&mut next);
        let next = Arc::new(next);
        match self.runners.write() {
            Ok(mut guard) => *guard = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    /// Install a runner, replacing any occupant of the same slot.
    pub fn install(&self, kind: RunnerKind, runner: Arc<dyn Runner>) {
        self.mutate(|runners| {
            runners.insert(kind, runner);
        });
    }

    /// Drop a runner. Sessions bound to it stay bound — they fail with "runner is not
    /// available" rather than being silently rerouted to an engine that never saw them.
    pub fn uninstall(&self, kind: &RunnerKind) {
        self.mutate(|runners| {
            runners.remove(kind);
        });
    }

    /// Whether a slot is currently served.
    pub fn has(&self, kind: &RunnerKind) -> bool {
        self.snapshot().contains_key(kind)
    }

    /// The runner serving a slot, lifted out of the map so the caller can await on it
    /// without borrowing the snapshot it came from.
    fn runner(&self, kind: &RunnerKind) -> Result<Arc<dyn Runner>> {
        self.snapshot()
            .get(kind)
            .cloned()
            .context("runner is not available")
    }

    pub fn default_kind(&self) -> RunnerKind {
        self.default.clone()
    }
    pub fn available(&self) -> Vec<RunnerKind> {
        let mut runners: Vec<_> = self.snapshot().keys().cloned().collect();
        runners.sort_by(|a, b| a.display_name().cmp(&b.display_name()));
        runners
    }

    pub fn event_endpoints(&self) -> Vec<(RunnerKind, String)> {
        self.snapshot()
            .iter()
            .filter_map(|(kind, runner)| runner.event_url().map(|url| (kind.clone(), url)))
            .collect()
    }

    pub fn event_receivers(&self) -> Vec<(RunnerKind, broadcast::Receiver<String>)> {
        self.snapshot()
            .iter()
            .filter_map(|(kind, runner)| {
                runner
                    .event_receiver()
                    .map(|receiver| (kind.clone(), receiver))
            })
            .collect()
    }

    /// The in-process event stream for one runner, for a runner installed after startup
    /// (the SSE fan-out is wired per runner, so a new one needs its own subscription).
    pub fn event_receiver_for(&self, kind: &RunnerKind) -> Option<broadcast::Receiver<String>> {
        self.snapshot().get(kind)?.event_receiver()
    }

    pub async fn has_binding(&self, session_id: &str) -> bool {
        self.bindings.read().await.contains_key(session_id)
    }

    /// Whether this id names a session opman already routes, at a usable location.
    /// Every runner allocates its session through the registry, so an id with no
    /// binding is one opman has never owned — there is nothing to adopt.
    pub async fn has_known_session(&self, session_id: &str, directory: &str) -> bool {
        validate_location(session_id, directory).is_ok() && self.has_binding(session_id).await
    }

    pub async fn runner_for(&self, session_id: &str) -> RunnerKind {
        self.bindings
            .read()
            .await
            .get(session_id)
            .map(|b| b.runner.clone())
            .unwrap_or_else(|| self.default.clone())
    }

    async fn binding(&self, session_id: &str, directory: &str) -> Binding {
        if let Some(binding) = self.bindings.read().await.get(session_id).cloned() {
            return binding;
        }
        let binding = Binding {
            runner: self.default.clone(),
            physical_id: session_id.to_string(),
            directory: directory.to_string(),
        };
        self.bindings
            .write()
            .await
            .insert(session_id.to_string(), binding.clone());
        binding
    }

    pub async fn messages(&self, session_id: &str, directory: &str) -> Result<Value> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(session_id.as_str(), directory).await;
        self.runner(&binding.runner)?
            .messages(&binding.physical_id, &binding.directory)
            .await
    }

    /// Return a runner-neutral snapshot used by the agent-manager MCP.
    pub async fn progress(&self, session_id: &str, directory: &str) -> Result<Value> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let session_id = session_id.to_string();
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(&session_id, directory).await;
        let runner = self.runner(&binding.runner)?;
        let status = runner
            .status(&binding.directory)
            .await
            .unwrap_or_else(|_| json!({}));
        let busy = status
            .get(&binding.physical_id)
            .is_some_and(is_running_status);
        let transcript = runner
            .messages(&binding.physical_id, &binding.directory)
            .await
            .unwrap_or_else(|_| json!([]));
        Ok(json!({
            "session_id": session_id,
            "physical_session_id": binding.physical_id,
            "directory": binding.directory,
            "runner": binding.runner,
            "busy": busy,
            "messages": recent_progress_messages(&transcript, 8),
        }))
    }

    /// Ask every runner which of its sessions are running right now.
    ///
    /// A session's turn is owned by one runner and only that runner knows it is
    /// under way, so the running set is the union across all of them. Each entry
    /// is the runner's display name plus the ids it reports; `None` marks a
    /// runner that could not be reached. That is deliberately distinct from "no
    /// sessions running" — a caller must not retire a session's running state on
    /// the word of a runner that never answered.
    pub async fn status_all(&self, directory: &str) -> Vec<(String, Option<HashSet<String>>)> {
        let Ok(directory) = ProjectDirectory::new(directory) else {
            return Vec::new();
        };
        let Some(directory) = directory.as_str() else {
            return Vec::new();
        };
        let runners = self.snapshot();
        let probes = runners.iter().map(|(kind, runner)| async move {
            let reported = runner.status(directory).await.ok();
            (
                kind.display_name().to_string(),
                reported.as_ref().map(running_session_ids),
            )
        });
        futures::future::join_all(probes).await
    }

    pub async fn sessions(
        &self,
        directory: &str,
    ) -> Result<Vec<(RunnerKind, crate::app::SessionInfo)>> {
        let directory = ProjectDirectory::new(directory)
            .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let mut all = Vec::new();
        let runners = self.snapshot();
        for (kind, runner) in runners.iter() {
            if kind == &self.default {
                continue;
            }
            let Ok(sessions) = runner.sessions(directory).await else {
                continue;
            };
            for session in sessions {
                self.bindings.write().await.insert(
                    session.id.clone(),
                    Binding {
                        runner: kind.clone(),
                        physical_id: session.id.clone(),
                        directory: directory.to_string(),
                    },
                );
                all.push((kind.clone(), session));
            }
        }
        Ok(all)
    }

    pub async fn create_session(
        &self,
        kind: RunnerKind,
        directory: &str,
        title: &str,
    ) -> Result<RunnerSession> {
        let directory = ProjectDirectory::new(directory)
            .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let runner = self.runner(&kind)?;
        let session = runner.create_session(directory, title).await?;
        self.bindings.write().await.insert(
            session.id.clone(),
            Binding {
                runner: kind,
                physical_id: session.id.clone(),
                directory: directory.to_string(),
            },
        );
        Ok(session)
    }

    /// Bind a session to the runner that owns it, without creating anything.
    ///
    /// Bindings live in memory, so a session created before this process started
    /// (or before it learned the session exists) has none, and `binding` would
    /// fall back to the default runner — turning the session's next send into a
    /// handoff it never asked for. Callers that know the owner from elsewhere
    /// (the web state's runner label, say) record it here first. Existing
    /// bindings win: they were established by a real create or handoff.
    pub async fn ensure_binding(&self, session_id: &str, kind: RunnerKind, directory: &str) {
        let Ok((session_id, directory)) = validate_location(session_id, directory) else {
            return;
        };
        let Some(directory) = directory.as_str() else {
            return;
        };
        let session_id = session_id.into_inner();
        self.bindings
            .write()
            .await
            .entry(session_id.clone())
            .or_insert(Binding {
                runner: kind,
                physical_id: session_id,
                directory: directory.to_string(),
            });
    }

    /// Ask one runner for a directory-scoped catalog it owns.
    ///
    /// The three catalogs — models, agents, commands — differ only in which trait method
    /// answers, so the validation lives here once instead of being re-typed per catalog.
    async fn catalog(
        &self,
        kind: RunnerKind,
        directory: &str,
        ask: impl for<'a> FnOnce(&'a dyn Runner, &'a str) -> RunnerFuture<'a, Value>,
    ) -> Result<Value> {
        let directory = ProjectDirectory::new(directory)
            .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let runner = self.runner(&kind)?;
        ask(runner.as_ref(), directory).await
    }

    pub async fn providers(&self, kind: RunnerKind, directory: &str) -> Result<Value> {
        self.catalog(kind, directory, |runner, dir| runner.providers(dir))
            .await
    }

    pub async fn agents(&self, kind: RunnerKind, directory: &str) -> Result<Value> {
        self.catalog(kind, directory, |runner, dir| runner.agents(dir))
            .await
    }

    /// Slash commands the runner advertises for this directory.
    pub async fn commands(&self, kind: RunnerKind, directory: &str) -> Result<Value> {
        self.catalog(kind, directory, |runner, dir| runner.commands(dir))
            .await
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        directory: &str,
        requested: Option<RunnerKind>,
        body: Value,
    ) -> Result<SendOutcome> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let logical_session_id = session_id.to_string();
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let current = self.binding(&logical_session_id, directory).await;
        let target_kind = requested.unwrap_or_else(|| current.runner.clone());
        let target = self
            .runner(&target_kind)
            .context("requested runner is not available")?;
        if target_kind == current.runner {
            let response = target
                .send_message(&current.physical_id, directory, body)
                .await?;
            return Ok(SendOutcome {
                session_id: logical_session_id,
                runner: target_kind,
                switched: false,
                response,
            });
        }

        let history = self
            .runner(&current.runner)
            .context("current runner is not available")?
            .messages(&current.physical_id, &current.directory)
            .await?;
        let transcript = crate::runner_handoff::render_transcript(&history);
        let user_text = extract_text(&body);
        let handoff = crate::runner_handoff::build_prompt(
            &current.runner.display_name(),
            &transcript,
            &user_text,
        );
        let session = target.create_session(directory, "Handoff session").await?;
        let mut handoff_body = body;
        handoff_body["parts"] = json!([{ "type": "text", "text": handoff }]);
        let response = target
            .send_message(&session.id, directory, handoff_body)
            .await?;
        self.bindings.write().await.insert(
            session.id.clone(),
            Binding {
                runner: target_kind.clone(),
                physical_id: session.id.clone(),
                directory: directory.to_string(),
            },
        );
        Ok(SendOutcome {
            session_id: session.id,
            runner: target_kind,
            switched: true,
            response,
        })
    }

    pub async fn abort(&self, session_id: &str, directory: &str) -> Result<()> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(session_id.as_str(), directory).await;
        self.runner(&binding.runner)?
            .abort(&binding.physical_id, &binding.directory)
            .await
    }

    pub async fn rename(&self, session_id: &str, title: &str, directory: &str) -> Result<bool> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(session_id.as_str(), directory).await;
        self.runner(&binding.runner)?
            .rename(&binding.physical_id, title)
            .await
    }

    pub async fn delete(&self, session_id: &str, directory: &str) -> Result<bool> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(session_id.as_str(), directory).await;
        let deleted = self
            .runner(&binding.runner)?
            .delete(&binding.physical_id)
            .await?;
        if deleted {
            self.bindings.write().await.remove(session_id.as_str());
        }
        Ok(deleted)
    }

    pub async fn reply_permission(&self, request_id: &str, reply: &str) -> Result<bool> {
        let runners = self.snapshot();
        for runner in runners.values() {
            if runner.reply_permission(request_id, reply).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn reply_question(&self, request_id: &str, answers: &[Vec<String>]) -> Result<bool> {
        let runners = self.snapshot();
        for runner in runners.values() {
            if runner.reply_question(request_id, answers).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn validate_location(session_id: &str, directory: &str) -> Result<(SessionId, ProjectDirectory)> {
    let session_id = SessionId::new(session_id)
        .map_err(|error| anyhow::anyhow!("invalid session id: {error}"))?;
    let directory = ProjectDirectory::new(directory)
        .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
    Ok((session_id, directory))
}

fn extract_text(body: &Value) -> String {
    body.get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn recent_progress_messages(body: &Value, limit: usize) -> Vec<Value> {
    let mut messages: Vec<Value> = if let Some(array) = body.as_array() {
        array.clone()
    } else if let Some(object) = body.as_object() {
        object.values().cloned().collect()
    } else {
        Vec::new()
    };
    messages.sort_by_key(|message| {
        message
            .pointer("/info/time/created")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    });
    messages.into_iter().rev().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn serve(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        url
    }

    /// Runners agree on the `/session/status` envelope but not on the word for
    /// "still going", and a retry is the same unfinished turn.
    #[test]
    fn running_status_accepts_every_runners_wording() {
        for kind in ["busy", "retry", "active"] {
            assert!(
                is_running_status(&json!({ "type": kind })),
                "{kind} should read as running"
            );
        }
        assert!(!is_running_status(&json!({ "type": "idle" })));
        assert!(!is_running_status(&json!({})));
        assert!(!is_running_status(&Value::Null));
    }

    #[test]
    fn running_session_ids_keeps_only_the_running_entries() {
        let status = json!({
            "a": { "type": "busy" },
            "b": { "type": "idle" },
            "c": { "type": "retry" },
        });
        let ids = running_session_ids(&status);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("a") && ids.contains("c"));
        assert!(running_session_ids(&json!([])).is_empty());
    }

    /// The union across runners is the point: asking only the default runner is
    /// what left every other runner's sessions stuck.
    #[tokio::test]
    async fn status_all_reports_each_runner_separately() {
        let opencode = serve(axum::Router::new().route(
            "/session/status",
            axum::routing::get(|| async { axum::Json(json!({ "s1": { "type": "busy" } })) }),
        ))
        .await;
        let client = reqwest::Client::new();
        let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
        runners.insert(
            RunnerKind::Opencode,
            Arc::new(HttpRunner::new(
                RunnerKind::Opencode,
                opencode,
                client.clone(),
            )),
        );
        // Nothing listens here, so this runner cannot answer.
        runners.insert(
            RunnerKind::ClaudeCode,
            Arc::new(HttpRunner::new(
                RunnerKind::ClaudeCode,
                "http://127.0.0.1:1",
                client,
            )),
        );
        let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);

        let reported: HashMap<String, Option<HashSet<String>>> =
            registry.status_all("/project").await.into_iter().collect();
        assert_eq!(reported.len(), 2);
        assert_eq!(
            reported["opencode"].as_ref().map(HashSet::len),
            Some(1),
            "the reachable runner reports its running session"
        );
        assert!(
            reported["claude-code"].is_none(),
            "an unreachable runner reports nothing, not an empty set"
        );
    }

    /// A reply is owned by exactly one engine. `ok:false`, transport errors, and
    /// missing routes must all read as "not ours" so the registry fan-out reaches
    /// the engine that actually raised the request.
    #[tokio::test]
    async fn http_runner_reply_routes_by_ownership() {
        use axum::routing::post;
        let owner = axum::Router::new().route(
            "/permission/{id}/reply",
            post(|| async { axum::Json(json!({ "ok": true })) }),
        );
        let stranger = axum::Router::new().route(
            "/permission/{id}/reply",
            post(|| async { axum::Json(json!({ "ok": false })) }),
        );
        let client = reqwest::Client::new();
        let owning = HttpRunner::new(RunnerKind::Claude, serve(owner).await, client.clone());
        let other = HttpRunner::new(
            RunnerKind::ClaudeCode,
            serve(stranger).await,
            client.clone(),
        );
        // No such route at all (native opencode shape) → not ours, not an error.
        let no_route = HttpRunner::new(
            RunnerKind::Opencode,
            serve(axum::Router::new()).await,
            client.clone(),
        );
        // Dead port → transport error also reads as not-ours.
        let dead = HttpRunner::new(RunnerKind::Opencode, "http://127.0.0.1:9", client);

        assert!(owning.reply_permission("p1", "once").await.unwrap());
        assert!(!other.reply_permission("p1", "once").await.unwrap());
        assert!(!no_route.reply_permission("p1", "once").await.unwrap());
        assert!(!dead.reply_permission("p1", "once").await.unwrap());
    }

    #[tokio::test]
    async fn http_runner_question_reply_posts_answers() {
        use axum::routing::post;
        let app = axum::Router::new().route(
            "/question/{id}/reply",
            post(|axum::Json(b): axum::Json<Value>| async move {
                let good = b["answers"][0][0] == "A" && b["answers"][1][0] == "B";
                axum::Json(json!({ "ok": good }))
            }),
        );
        let runner = HttpRunner::new(RunnerKind::Claude, serve(app).await, reqwest::Client::new());
        let answers = vec![vec!["A".to_string()], vec!["B".to_string()]];
        assert!(runner.reply_question("q1", &answers).await.unwrap());
    }

    #[test]
    fn parses_runner_names() {
        assert_eq!(
            RunnerKind::parse("claude-code"),
            Some(RunnerKind::ClaudeCode)
        );
        assert_eq!(RunnerKind::parse("nope"), None);
        // Codex is no longer a compile-time runner: it reaches opman as the ACP agent
        // `acp.json` declares, so its label parses only once that config registered it.
        register_acp_runners(["codex".to_string()]);
        assert_eq!(
            RunnerKind::parse("codex"),
            Some(RunnerKind::Acp("codex".to_string()))
        );
    }

    /// Sends must go to `prompt_async`, not `/message`.
    ///
    /// `POST /session/{id}/message` only responds once the assistant has
    /// finished, so awaiting it holds the caller's request open for the whole
    /// turn — a long turn then reads as a hang and dies on the client or tunnel
    /// timeout. This test stands up a server that would block forever on
    /// `/message`: if the endpoint ever regresses, it hangs instead of passing.
    #[tokio::test]
    async fn send_message_uses_the_non_blocking_prompt_endpoint() {
        use std::sync::Arc as StdArc;
        use tokio::sync::Mutex as AsyncMutex;

        let hits: StdArc<AsyncMutex<Vec<String>>> = StdArc::new(AsyncMutex::new(vec![]));
        let seen = hits.clone();
        let app = axum::Router::new()
            .route(
                "/session/{id}/prompt_async",
                axum::routing::post(
                    move |axum::extract::Path(id): axum::extract::Path<String>| {
                        let seen = seen.clone();
                        async move {
                            seen.lock().await.push(format!("prompt_async:{id}"));
                            axum::Json(json!({ "ok": true }))
                        }
                    },
                ),
            )
            .route(
                "/session/{id}/message",
                axum::routing::post(|| async {
                    // Stand in for the streaming endpoint: never responds.
                    std::future::pending::<()>().await;
                    axum::Json(json!({}))
                }),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let runner = HttpRunner::new(RunnerKind::Opencode, base, reqwest::Client::new());
        let body = json!({ "parts": [{ "type": "text", "text": "hi" }] });
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            runner.send_message("s1", "/project", body),
        )
        .await
        .expect("send_message hit the blocking /message endpoint")
        .expect("send should succeed");

        assert_eq!(out["ok"], true);
        assert_eq!(hits.lock().await.as_slice(), ["prompt_async:s1"]);
    }

    /// The OpenCode body rewrite must survive the endpoint change: runner-only
    /// controls are stripped and `effort` becomes `variant`.
    #[tokio::test]
    async fn opencode_send_strips_runner_controls_and_maps_effort() {
        use std::sync::Arc as StdArc;
        use tokio::sync::Mutex as AsyncMutex;

        let seen: StdArc<AsyncMutex<Option<Value>>> = StdArc::new(AsyncMutex::new(None));
        let sink = seen.clone();
        let app = axum::Router::new().route(
            "/session/{id}/prompt_async",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let sink = sink.clone();
                async move {
                    *sink.lock().await = Some(body);
                    axum::Json(json!({ "ok": true }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let runner = HttpRunner::new(RunnerKind::Opencode, base, reqwest::Client::new());
        runner
            .send_message(
                "s1",
                "/project",
                json!({
                    "parts": [{ "type": "text", "text": "hi" }],
                    "effort": "high",
                    "permission": "default",
                    "runner": "opencode",
                    "agent": "claude",
                }),
            )
            .await
            .expect("send should succeed");

        let body = seen.lock().await.clone().expect("body was forwarded");
        assert_eq!(body["variant"], "high");
        assert!(body.get("effort").is_none());
        assert!(body.get("permission").is_none());
        assert!(body.get("runner").is_none());
        // "claude" is an opman-side label, not an OpenCode agent.
        assert!(body.get("agent").is_none());
    }

    struct MockRunner {
        kind: RunnerKind,
        prefix: &'static str,
        next: AtomicUsize,
        sessions: RwLock<HashMap<String, Value>>,
    }

    impl Runner for MockRunner {
        fn kind(&self) -> RunnerKind {
            self.kind.clone()
        }
        fn create_session<'a>(
            &'a self,
            _directory: &'a str,
            title: &'a str,
        ) -> RunnerFuture<'a, RunnerSession> {
            Box::pin(async move {
                let id = format!(
                    "{}_{}",
                    self.prefix,
                    self.next.fetch_add(1, Ordering::Relaxed)
                );
                self.sessions.write().await.insert(id.clone(), json!([]));
                Ok(RunnerSession {
                    id,
                    title: title.to_string(),
                })
            })
        }
        fn messages<'a>(
            &'a self,
            session_id: &'a str,
            _directory: &'a str,
        ) -> RunnerFuture<'a, Value> {
            Box::pin(async move {
                Ok(self
                    .sessions
                    .read()
                    .await
                    .get(session_id)
                    .cloned()
                    .unwrap_or(json!([])))
            })
        }
        fn send_message<'a>(
            &'a self,
            session_id: &'a str,
            _directory: &'a str,
            body: Value,
        ) -> RunnerFuture<'a, Value> {
            Box::pin(async move {
                self.sessions
                    .write()
                    .await
                    .entry(session_id.to_string())
                    .or_insert(json!([]));
                Ok(body)
            })
        }
        fn abort<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn switching_runner_creates_a_handoff_session_with_summary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old = Arc::new(MockRunner {
            kind: RunnerKind::Opencode,
            prefix: "old",
            next: AtomicUsize::new(1),
            sessions: RwLock::new(HashMap::new()),
        });
        old.sessions.write().await.insert(
            "logical".into(),
            json!([
                { "info": { "role": "user" }, "parts": [{ "text": "Fix the parser" }] },
                { "info": { "role": "assistant" }, "parts": [{ "text": "I found the parser" }] }
            ]),
        );
        let new = Arc::new(MockRunner {
            kind: RunnerKind::ClaudeCode,
            prefix: "new",
            next: AtomicUsize::new(1),
            sessions: RwLock::new(HashMap::new()),
        });
        let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
        runners.insert(RunnerKind::Opencode, old.clone());
        runners.insert(RunnerKind::ClaudeCode, new.clone());
        let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);
        let outcome = registry
            .send_message(
                "logical",
                "/project",
                Some(RunnerKind::ClaudeCode),
                json!({
                    "parts": [{ "type": "text", "text": "Now add a regression test" }]
                }),
            )
            .await?;
        assert!(outcome.switched);
        assert_eq!(outcome.runner, RunnerKind::ClaudeCode);
        assert!(outcome.session_id.starts_with("new_"));
        let handoff = outcome.response["parts"][0]["text"]
            .as_str()
            .ok_or("handoff response did not contain text")?;
        assert!(handoff.contains("Fix the parser"));
        Ok(())
    }

    fn two_runner_registry() -> (Arc<MockRunner>, Arc<MockRunner>, RunnerRegistry) {
        let default = Arc::new(MockRunner {
            kind: RunnerKind::ClaudeCode,
            prefix: "cc",
            next: AtomicUsize::new(1),
            sessions: RwLock::new(HashMap::new()),
        });
        let other = Arc::new(MockRunner {
            kind: RunnerKind::Claude,
            prefix: "cp",
            next: AtomicUsize::new(1),
            sessions: RwLock::new(HashMap::new()),
        });
        let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
        runners.insert(RunnerKind::ClaudeCode, default.clone());
        runners.insert(RunnerKind::Claude, other.clone());
        let registry = RunnerRegistry::new(RunnerKind::ClaudeCode, runners);
        (default, other, registry)
    }

    /// The follow-up turn of an ordinary conversation must land in the same
    /// session. A send that names no runner is not a switch request, so it stays
    /// on whatever runner the session is bound to.
    #[tokio::test]
    async fn sends_without_a_requested_runner_stay_on_the_bound_runner(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_default, other, registry) = two_runner_registry();
        let created = registry
            .create_session(RunnerKind::Claude, "/project", "chat")
            .await?;

        for turn in ["first", "second"] {
            let outcome = registry
                .send_message(
                    &created.id,
                    "/project",
                    None,
                    json!({ "parts": [{ "type": "text", "text": turn }] }),
                )
                .await?;
            assert!(!outcome.switched, "turn {turn} forked the session");
            assert_eq!(outcome.session_id, created.id);
            assert_eq!(outcome.runner, RunnerKind::Claude);
        }
        // The default runner never saw the conversation.
        assert!(other.sessions.read().await.contains_key(&created.id));
        Ok(())
    }

    /// Naming the runner the session already uses is not a switch either — the
    /// UI may legitimately restate it on a new session's first turn.
    #[tokio::test]
    async fn restating_the_bound_runner_is_not_a_switch() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_default, _other, registry) = two_runner_registry();
        let created = registry
            .create_session(RunnerKind::Claude, "/project", "chat")
            .await?;
        let outcome = registry
            .send_message(
                &created.id,
                "/project",
                Some(RunnerKind::Claude),
                json!({ "parts": [{ "type": "text", "text": "hi" }] }),
            )
            .await?;
        assert!(!outcome.switched);
        assert_eq!(outcome.session_id, created.id);
        Ok(())
    }

    /// Bindings are in-memory, so a session opman learned about from somewhere
    /// else (its runner label, the session poller) has none. Without adopting
    /// it, the blind default-runner fallback turns its next turn into a handoff.
    #[tokio::test]
    async fn ensure_binding_adopts_a_session_without_handing_it_off(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_default, other, registry) = two_runner_registry();
        other
            .sessions
            .write()
            .await
            .insert("orphan".into(), json!([]));

        registry
            .ensure_binding("orphan", RunnerKind::Claude, "/project")
            .await;
        let outcome = registry
            .send_message(
                "orphan",
                "/project",
                None,
                json!({ "parts": [{ "type": "text", "text": "resume" }] }),
            )
            .await?;
        assert!(!outcome.switched);
        assert_eq!(outcome.runner, RunnerKind::Claude);
        assert_eq!(outcome.session_id, "orphan");

        // Adoption never overrides an established binding.
        registry
            .ensure_binding("orphan", RunnerKind::ClaudeCode, "/project")
            .await;
        assert_eq!(registry.runner_for("orphan").await, RunnerKind::Claude);
        Ok(())
    }

    /// Without adoption the same send is read as "switch to the default runner"
    /// and forks the conversation — the regression this pair of tests guards.
    #[tokio::test]
    async fn an_unadopted_session_still_hands_off_when_a_runner_is_named(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_default, other, registry) = two_runner_registry();
        other
            .sessions
            .write()
            .await
            .insert("orphan".into(), json!([]));
        let outcome = registry
            .send_message(
                "orphan",
                "/project",
                Some(RunnerKind::Claude),
                json!({ "parts": [{ "type": "text", "text": "resume" }] }),
            )
            .await?;
        assert!(outcome.switched);
        assert_ne!(outcome.session_id, "orphan");
        Ok(())
    }
}
