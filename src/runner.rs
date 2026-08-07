//! Common runner abstraction used by the web backend.
//!
//! A session is deliberately kept separate from a runner.  This is what lets a
//! client select a different runner for the next turn: the registry creates a
//! new runner-native session, sends a handoff context to it, and returns the
//! new session id to the client.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{broadcast, oneshot, Mutex, OnceCell, RwLock};

#[path = "codex_history.rs"]
mod codex_history;

pub type RunnerFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub use opman_backend_contracts::{register_acp_runners, ProjectDirectory, RunnerKind, SessionId};

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
/// `retry`, ACP agents say `active`, and codex says `working`. A retry is still
/// the same unfinished turn, so it counts as running. An entry without a
/// recognised type is idle — the map only lists non-idle sessions anyway.
pub fn is_running_status(entry: &Value) -> bool {
    entry
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "busy" | "retry" | "working" | "active"))
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

#[derive(Clone, Debug)]
pub struct RunnerSession {
    pub id: String,
    pub title: String,
    pub directory: String,
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
                directory: directory.to_string(),
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
                        Some("default") | Some("claude") | Some("codex")
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

/// JSON-RPC transport for Codex's native app-server protocol.
struct CodexConnection {
    _child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>,
    next_id: AtomicU64,
    events: broadcast::Sender<Value>,
}

impl Drop for CodexConnection {
    fn drop(&mut self) {
        if let Ok(mut child) = self._child.try_lock() {
            let _ = child.start_kill();
        }
    }
}

impl CodexConnection {
    async fn start(binary: &str) -> Result<Arc<Self>> {
        let mut child = tokio::process::Command::new(binary)
            .args(["app-server", "--stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to start codex app-server")?;
        let stdin = child
            .stdin
            .take()
            .context("codex app-server stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("codex app-server stdout unavailable")?;
        let stderr = child
            .stderr
            .take()
            .context("codex app-server stderr unavailable")?;
        let (events, _) = broadcast::channel(2048);
        let connection = Arc::new(Self {
            _child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            events,
        });

        let reader_connection = connection.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let is_response = message.get("result").is_some() || message.get("error").is_some();
                if is_response {
                    if let Some(id) = rpc_id_key(message.get("id")) {
                        if let Some(waiter) = reader_connection.pending.lock().await.remove(&id) {
                            let result = if let Some(error) = message.get("error") {
                                Err(error.to_string())
                            } else {
                                Ok(message.get("result").cloned().unwrap_or(Value::Null))
                            };
                            let _ = waiter.send(result);
                        }
                        continue;
                    }
                }
                let _ = reader_connection.events.send(message);
            }
        });
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while lines.next_line().await.ok().flatten().is_some() {}
        });

        let _ = connection
            .request(
                "initialize",
                json!({
                    "clientInfo": { "name": "opman", "title": "OpMan", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": { "experimentalApi": true }
                }),
            )
            .await?;
        connection.notify("initialized", json!({})).await?;
        Ok(connection)
    }

    async fn write_message(&self, message: Value) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(serde_json::to_string(&message)?.as_bytes())
            .await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write_message(json!({ "method": method, "params": params }))
            .await
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        if let Err(error) = self
            .write_message(json!({ "id": id, "method": method, "params": params }))
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        rx.await
            .context("codex app-server response channel closed")?
            .map_err(|error| anyhow::anyhow!("codex app-server error: {error}"))
    }

    fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    async fn respond(&self, id: Value, result: Value) -> Result<()> {
        self.write_message(json!({ "id": id, "result": result }))
            .await
    }
}

fn rpc_id_key(id: Option<&Value>) -> Option<String> {
    id.map(|value| {
        value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string())
    })
}

struct CodexSessionState {
    title: String,
    directory: String,
    active_turn: Option<String>,
    model: Option<String>,
}

struct CodexPendingRequest {
    rpc_id: Value,
    method: String,
    params: Value,
}

struct CodexRuntime {
    binary: String,
    mcp: crate::mcp_registry::SharedRegistry,
    connection: OnceCell<Arc<CodexConnection>>,
    sessions: RwLock<HashMap<String, CodexSessionState>>,
    pending_requests: Mutex<HashMap<String, CodexPendingRequest>>,
    tool_outputs: Mutex<HashMap<String, String>>,
    events: broadcast::Sender<String>,
    event_task_started: AtomicU64,
}

/// The `config` blob Codex takes with `thread/start` and `thread/resume`.
///
/// `session` is `None` at `thread/start`, which is before Codex has allocated a thread
/// id — servers that route by session simply drop that pair there and get it on the
/// re-send once the id exists.
fn codex_config(
    handle: &crate::mcp_registry::RegistryHandle,
    directory: &str,
    session: Option<&str>,
) -> Value {
    let registry = handle.current();
    crate::mcp_registry::render::codex_thread_config(
        registry.for_runner(&RunnerKind::Codex),
        registry.bind(directory, session),
    )
}

/// Full Codex app-server adapter. It keeps one long-lived JSON-RPC process,
/// persists threads through Codex's own rollout store, and translates native
/// notifications into the OpenCode-shaped event stream consumed by OpMan.
pub struct CodexRunner {
    runtime: Arc<CodexRuntime>,
}

impl CodexRunner {
    pub fn new(_client: reqwest::Client, mcp: crate::mcp_registry::SharedRegistry) -> Self {
        let (events, _) = broadcast::channel(2048);
        Self {
            runtime: Arc::new(CodexRuntime {
                binary: std::env::var("OPMAN_CODEX_BIN").unwrap_or_else(|_| "codex".to_string()),
                mcp,
                connection: OnceCell::const_new(),
                sessions: RwLock::new(HashMap::new()),
                pending_requests: Mutex::new(HashMap::new()),
                tool_outputs: Mutex::new(HashMap::new()),
                events,
                event_task_started: AtomicU64::new(0),
            }),
        }
    }

    async fn connection(&self) -> Result<Arc<CodexConnection>> {
        self.runtime.connection().await
    }

    async fn resume_thread(
        &self,
        connection: &CodexConnection,
        session_id: &str,
        directory: &str,
    ) -> Result<()> {
        if self.runtime.sessions.read().await.contains_key(session_id) {
            return Ok(());
        }
        let response = connection
            .request(
                "thread/resume",
                json!({
                    "threadId": session_id,
                    "cwd": directory,
                    "config": codex_config(&self.runtime.mcp, directory, Some(session_id)),
                }),
            )
            .await?;
        let thread = response
            .get("thread")
            .context("codex did not return the resumed thread")?;
        let title = thread
            .get("name")
            .and_then(Value::as_str)
            .filter(|title| !title.is_empty())
            .unwrap_or("Codex session")
            .to_string();
        let model = thread
            .get("model")
            .and_then(Value::as_str)
            .or_else(|| thread.get("modelProvider").and_then(Value::as_str))
            .map(str::to_string);
        let mut sessions = self.runtime.sessions.write().await;
        let session = sessions
            .entry(session_id.to_string())
            .or_insert(CodexSessionState {
                title: title.clone(),
                directory: directory.to_string(),
                active_turn: None,
                model: model.clone(),
            });
        session.title = title;
        session.directory = directory.to_string();
        if model.is_some() {
            session.model = model;
        }
        Ok(())
    }
}

impl CodexRuntime {
    async fn connection(self: &Arc<Self>) -> Result<Arc<CodexConnection>> {
        let connection = self
            .connection
            .get_or_try_init(|| async { CodexConnection::start(&self.binary).await })
            .await?
            .clone();
        if self
            .event_task_started
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let runtime = self.clone();
            let connection_for_task = connection.clone();
            tokio::spawn(async move {
                runtime.process_events(connection_for_task).await;
            });
        }
        Ok(connection)
    }

    async fn process_events(self: Arc<Self>, connection: Arc<CodexConnection>) {
        let mut receiver = connection.subscribe();
        while let Ok(event) = receiver.recv().await {
            self.process_event(&connection, event).await;
        }
    }

    async fn process_event(&self, connection: &CodexConnection, event: Value) {
        let Some(method) = event.get("method").and_then(Value::as_str) else {
            return;
        };
        let params = event.get("params").cloned().unwrap_or(Value::Null);
        if let Some(rpc_id) = rpc_id_key(event.get("id")) {
            self.pending_requests.lock().await.insert(
                rpc_id.clone(),
                CodexPendingRequest {
                    rpc_id: event.get("id").cloned().unwrap_or(Value::Null),
                    method: method.to_string(),
                    params: params.clone(),
                },
            );
            self.emit_server_request(method, &params, &rpc_id).await;
            return;
        }

        match method {
            "thread/status/changed" => {
                let session_id = string_at(&params, "threadId");
                let status = params
                    .pointer("/status/type")
                    .and_then(Value::as_str)
                    .unwrap_or("idle");
                self.emit(json!({
                    "type": "session.status",
                    "properties": { "sessionID": session_id, "status": { "type": if status == "active" { "busy" } else { "idle" } } }
                })).await;
            }
            "turn/started" => {
                let session_id = string_at(&params, "threadId");
                let turn_id = params
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !session_id.is_empty() {
                    if let Some(session) = self.sessions.write().await.get_mut(&session_id) {
                        session.active_turn = Some(turn_id);
                    }
                    self.emit(json!({ "type": "session.status", "properties": { "sessionID": session_id, "status": { "type": "busy" } } })).await;
                }
            }
            "item/started" => self.emit_item_event(&params, false).await,
            "item/completed" => self.emit_item_event(&params, true).await,
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                self.emit_reasoning_delta(&params).await
            }
            "item/agentMessage/delta" => {
                self.emit(json!({
                    "type": "message.part.delta",
                    "properties": {
                        "sessionID": string_at(&params, "threadId"),
                        "messageID": string_at(&params, "itemId"),
                        "partID": string_at(&params, "itemId"),
                        "field": "text",
                        "delta": params.get("delta").and_then(Value::as_str).unwrap_or("")
                    }
                }))
                .await;
            }
            "item/commandExecution/outputDelta" => self.emit_tool_delta(&params, "bash").await,
            "item/commandExecution/terminalInteraction" => {
                self.emit_tool_delta(&params, "bash").await
            }
            "item/fileChange/outputDelta" => self.emit_tool_delta(&params, "edit").await,
            "item/fileChange/patchUpdated" => self.emit_tool_patch(&params).await,
            "item/mcpToolCall/progress" => self.emit_tool_delta(&params, "mcp").await,
            "thread/tokenUsage/updated" => self.emit_usage(&params).await,
            "turn/completed" => {
                let session_id = string_at(&params, "threadId");
                let turn_id = params
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if params.pointer("/turn/status").and_then(Value::as_str) == Some("failed") {
                    let message = params
                        .pointer("/turn/error")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "Codex turn failed".to_string());
                    self.emit(json!({ "type": "session.error", "properties": { "sessionID": session_id, "error": message } })).await;
                }
                if let Some(session) = self.sessions.write().await.get_mut(&session_id) {
                    if session.active_turn.as_deref() == Some(&turn_id) {
                        session.active_turn = None;
                    }
                }
                self.emit(json!({ "type": "session.status", "properties": { "sessionID": session_id, "status": { "type": "idle" } } })).await;
            }
            "thread/name/updated" => {
                self.emit(json!({
                    "type": "session.updated",
                    "properties": { "info": { "id": string_at(&params, "threadId"), "title": params.get("name").cloned().unwrap_or(Value::Null) } }
                })).await;
            }
            _ => {}
        }

        // Keep the native event available to future runner-specific consumers.
        let _ = connection;
    }

    async fn emit_server_request(&self, method: &str, params: &Value, request_id: &str) {
        let session_id = string_at(params, "threadId");
        match method {
            "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval" => {
                self.emit(json!({
                    "type": "permission.asked",
                    "properties": {
                        "id": request_id,
                        "sessionID": session_id,
                        "permission": if method.contains("fileChange") { "edit" } else { "bash" },
                        "description": params.get("reason").and_then(Value::as_str).or_else(|| params.get("command").and_then(Value::as_str)).unwrap_or("Codex requests approval"),
                        "metadata": params
                    }
                })).await;
            }
            "item/tool/requestUserInput" => {
                let questions = params
                    .get("questions")
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                self.emit(json!({
                    "type": "question.asked",
                    "properties": { "id": request_id, "sessionID": session_id, "questions": questions, "title": "Codex input required" }
                })).await;
            }
            _ => {}
        }
    }

    async fn emit_item_event(&self, params: &Value, completed: bool) {
        let session_id = string_at(params, "threadId");
        let item = params.get("item").cloned().unwrap_or(Value::Null);
        let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        let timestamp = params
            .get(if completed {
                "completedAtMs"
            } else {
                "startedAtMs"
            })
            .cloned()
            .unwrap_or(json!(chrono::Utc::now().timestamp_millis()));
        match item_type {
            "userMessage" => {
                let text = item
                    .pointer("/content/0/text")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let info = json!({ "id": item_id, "messageID": item_id, "sessionID": session_id, "role": "user", "time": { "created": timestamp } });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } }))
                    .await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": format!("{item_id}_text"), "sessionID": session_id, "messageID": item_id, "type": "text", "text": text } } })).await;
            }
            "reasoning" | "analysis" | "thinking" => {
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("summary").and_then(Value::as_str))
                    .unwrap_or("");
                let message_id = format!("turn_{}", string_at(params, "turnId"));
                let info = json!({ "id": message_id, "messageID": message_id, "sessionID": session_id, "role": "assistant", "agent": "codex", "time": { "created": timestamp } });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } }))
                    .await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "reasoning", "text": text } } })).await;
            }
            "agentMessage" => {
                let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                let info = json!({ "id": item_id, "messageID": item_id, "sessionID": session_id, "role": "assistant", "agent": "codex", "time": { "created": timestamp }, "modelID": self.sessions.read().await.get(&session_id).and_then(|s| s.model.clone()) });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } }))
                    .await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": item_id, "type": "text", "text": text } } })).await;
            }
            _ => {
                let Some(tool_part) = codex_tool_part(item_type, &item) else {
                    return;
                };
                let message_id = format!("turn_{}", string_at(params, "turnId"));
                let state = json!({
                    "status": if completed { if tool_part.error.is_some() { "error" } else { "completed" } } else { "running" },
                    "input": tool_part.input,
                    "output": tool_part.output.map(Value::String).unwrap_or(Value::Null),
                    "error": tool_part.error.map(Value::String).unwrap_or(Value::Null)
                });
                let info = json!({ "id": message_id, "messageID": message_id, "sessionID": session_id, "role": "assistant", "agent": "codex", "time": { "created": timestamp } });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } }))
                    .await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "tool", "tool": tool_part.tool, "callID": item_id, "state": state } } })).await;
            }
        }
    }

    async fn emit_reasoning_delta(&self, params: &Value) {
        let Some(delta) = params
            .get("delta")
            .and_then(Value::as_str)
            .or_else(|| params.get("text").and_then(Value::as_str))
        else {
            return;
        };
        if delta.is_empty() {
            return;
        }
        let session_id = string_at(params, "threadId");
        let message_id = format!("turn_{}", string_at(params, "turnId"));
        let part_id = string_at(params, "itemId");
        self.emit(json!({ "type": "message.updated", "properties": { "info": { "id": message_id, "messageID": message_id, "sessionID": session_id, "role": "assistant" } } })).await;
        self.emit(json!({ "type": "message.part.delta", "properties": { "sessionID": session_id, "messageID": message_id, "partID": part_id, "field": "text", "type": "reasoning", "delta": delta } })).await;
    }

    async fn emit_tool_delta(&self, params: &Value, tool: &str) {
        let item_id = string_at(params, "itemId");
        if item_id.is_empty() {
            return;
        }
        let Some(delta) = params
            .get("delta")
            .and_then(Value::as_str)
            .or_else(|| params.get("message").and_then(Value::as_str))
            .or_else(|| params.get("stdin").and_then(Value::as_str))
        else {
            return;
        };
        if delta.is_empty() {
            return;
        }
        let mut outputs = self.tool_outputs.lock().await;
        let output = outputs.entry(item_id.clone()).or_default();
        output.push_str(delta);
        let session_id = string_at(params, "threadId");
        let message_id = format!("turn_{}", string_at(params, "turnId"));
        self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "tool", "tool": tool, "callID": item_id, "state": { "status": "running", "output": output } } } })).await;
    }

    async fn emit_tool_patch(&self, params: &Value) {
        let item_id = string_at(params, "itemId");
        let Some(changes) = params
            .get("changes")
            .or_else(|| params.get("patch"))
            .cloned()
        else {
            return;
        };
        let session_id = string_at(params, "threadId");
        let message_id = format!("turn_{}", string_at(params, "turnId"));
        self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "tool", "tool": "edit", "callID": item_id, "state": { "status": "running", "input": { "changes": changes } } } } })).await;
    }

    async fn emit_usage(&self, params: &Value) {
        let usage = params
            .pointer("/tokenUsage/last")
            .cloned()
            .unwrap_or(Value::Null);
        let session_id = string_at(params, "threadId");
        self.emit(json!({ "type": "message.updated", "properties": { "info": { "id": format!("usage_{}", string_at(params, "turnId")), "messageID": format!("usage_{}", string_at(params, "turnId")), "sessionID": session_id, "role": "assistant", "agent": "codex", "tokens": { "input": usage.get("inputTokens").cloned().unwrap_or(json!(0)), "output": usage.get("outputTokens").cloned().unwrap_or(json!(0)), "reasoning": usage.get("reasoningOutputTokens").cloned().unwrap_or(json!(0)), "cache": { "read": usage.get("cachedInputTokens").cloned().unwrap_or(json!(0)) } } } } })).await;
    }

    async fn emit(&self, event: Value) {
        let _ = self.events.send(event.to_string());
    }
}

fn string_at(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

struct CodexToolPart {
    tool: String,
    input: Value,
    output: Option<String>,
    error: Option<String>,
}

fn codex_tool_part(item_type: &str, item: &Value) -> Option<CodexToolPart> {
    let (tool, input, output) = match item_type {
        "commandExecution" => (
            "bash".to_string(),
            json!({ "command": item.get("command"), "cwd": item.get("cwd"), "commandActions": item.get("commandActions") }),
            item.get("aggregatedOutput").and_then(display_value),
        ),
        "fileChange" => (
            "edit".to_string(),
            json!({ "changes": item.get("changes") }),
            item.get("output").and_then(display_value),
        ),
        "mcpToolCall" => (
            format!(
                "mcp_{}_{}",
                slug(item.get("server")),
                slug(item.get("tool").or_else(|| item.get("name")))
            ),
            item.get("arguments").cloned().unwrap_or(Value::Null),
            item.get("result").and_then(display_value),
        ),
        "dynamicToolCall" => (
            format!(
                "dynamic_{}_{}",
                slug(item.get("namespace")),
                slug(item.get("name").or_else(|| item.get("tool")))
            ),
            item.get("arguments").cloned().unwrap_or(Value::Null),
            item.get("contentItems").and_then(display_value),
        ),
        "collabAgentToolCall" => (
            "task".to_string(),
            json!({ "prompt": item.get("prompt"), "receiverThreadIds": item.get("receiverThreadIds") }),
            item.get("output").and_then(display_value),
        ),
        "subAgentActivity" => (
            "task".to_string(),
            json!({ "agentThreadId": item.get("agentThreadId"), "path": item.get("path"), "kind": item.get("kind") }),
            item.get("message").and_then(display_value),
        ),
        "webSearch" => (
            "web_search".to_string(),
            json!({ "query": item.get("query"), "action": item.get("action") }),
            item.get("results").and_then(display_value),
        ),
        "imageView" => (
            "image_view".to_string(),
            json!({ "path": item.get("path") }),
            None,
        ),
        "sleep" => (
            "sleep".to_string(),
            json!({ "durationMs": item.get("durationMs") }),
            None,
        ),
        "imageGeneration" => (
            "image_generation".to_string(),
            json!({ "prompt": item.get("revisedPrompt").or_else(|| item.get("prompt")) }),
            item.get("savedPath")
                .and_then(display_value)
                .or_else(|| item.get("result").and_then(display_value)),
        ),
        "plan" => (
            "plan".to_string(),
            json!({ "text": item.get("text") }),
            None,
        ),
        _ => return None,
    };
    let error = item
        .get("error")
        .and_then(display_value)
        .or_else(|| {
            (item
                .get("exitCode")
                .and_then(Value::as_i64)
                .is_some_and(|code| code != 0))
            .then(|| "Command exited with a non-zero status".to_string())
        })
        .or_else(|| {
            (item.get("success").and_then(Value::as_bool) == Some(false))
                .then(|| "Tool call failed".to_string())
        })
        .or_else(|| {
            matches!(
                item.get("status").and_then(Value::as_str),
                Some("failed" | "declined" | "error")
            )
            .then(|| "Tool call failed".to_string())
        });
    Some(CodexToolPart {
        tool,
        input,
        output,
        error,
    })
}

fn slug(value: Option<&Value>) -> String {
    let Some(value) = value.and_then(Value::as_str) else {
        return "tool".to_string();
    };
    let slug: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    if slug.is_empty() {
        "tool".to_string()
    } else {
        slug
    }
}

fn display_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        _ => serde_json::to_string_pretty(value).ok(),
    }
}

fn codex_session_info(thread: &Value) -> Option<crate::app::SessionInfo> {
    let id = thread.get("id").and_then(Value::as_str)?.to_string();
    let directory = thread.get("cwd").and_then(Value::as_str)?.to_string();
    let created = thread
        .get("createdAt")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_mul(1000);
    let updated = thread
        .get("updatedAt")
        .and_then(Value::as_u64)
        .or_else(|| thread.get("recencyAt").and_then(Value::as_u64))
        .unwrap_or(created / 1000)
        .saturating_mul(1000);
    Some(crate::app::SessionInfo {
        id,
        title: thread
            .get("name")
            .and_then(Value::as_str)
            .filter(|title| !title.is_empty())
            .or_else(|| thread.get("preview").and_then(Value::as_str))
            .unwrap_or("Codex session")
            .to_string(),
        parent_id: thread
            .get("parentThreadId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        directory,
        time: crate::app::SessionTime { created, updated },
    })
}

fn codex_input(body: &Value) -> Vec<Value> {
    body.get("parts").and_then(Value::as_array).into_iter().flatten().filter_map(|part| {
        match part.get("type").and_then(Value::as_str).unwrap_or("text") {
            "text" => Some(json!({ "type": "text", "text": part.get("text").and_then(Value::as_str).unwrap_or(""), "text_elements": [] })),
            "file" if part.get("mime").and_then(Value::as_str).unwrap_or("").starts_with("image/") => Some(json!({ "type": "image", "url": part.get("url").cloned().unwrap_or(Value::Null) })),
            _ => None,
        }
    }).collect()
}

fn dedupe_codex_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    messages
        .into_iter()
        .filter(|message| {
            let role = message
                .pointer("/info/role")
                .and_then(Value::as_str)
                .unwrap_or("");
            let created = message
                .pointer("/info/time/created")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let parts = message
                .get("parts")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|part| {
                    json!({
                        "type": part.get("type"),
                        "text": part.get("text"),
                        "tool": part.get("tool"),
                        "callID": part.get("callID"),
                        "state": part.get("state"),
                    })
                })
                .collect::<Vec<_>>();
            let key = serde_json::to_string(&(role, created, parts))
                .unwrap_or_else(|_| message.to_string());
            seen.insert(key)
        })
        .collect()
}

fn codex_messages(thread: &Value) -> Value {
    let thread_id = thread.get("id").and_then(Value::as_str).unwrap_or("");
    let turns = thread
        .get("turns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut messages = Vec::new();
    for turn in turns {
        let created = turn.get("startedAt").and_then(Value::as_i64).unwrap_or(0) * 1000;
        let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or("");
        for item in turn
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            let item_id = item.get("id").and_then(Value::as_str).unwrap_or("");
            match item_type {
                "userMessage" => {
                    let text = item
                        .pointer("/content/0/text")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    messages.push(json!({ "info": { "id": item_id, "messageID": item_id, "sessionID": thread_id, "role": "user", "time": { "created": created } }, "parts": [{ "id": format!("{item_id}_text"), "sessionID": thread_id, "messageID": item_id, "type": "text", "text": text }] }));
                }
                "reasoning" | "analysis" | "thinking" => {
                    let text = item
                        .get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("summary").and_then(Value::as_str))
                        .unwrap_or("");
                    messages.push(json!({ "info": { "id": item_id, "messageID": item_id, "sessionID": thread_id, "role": "assistant", "agent": "codex", "time": { "created": created } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": item_id, "type": "reasoning", "text": text }] }));
                }
                "agentMessage" => {
                    let status = if turn.get("status").and_then(Value::as_str) == Some("failed") {
                        "error"
                    } else {
                        "completed"
                    };
                    messages.push(json!({ "info": { "id": item_id, "messageID": item_id, "sessionID": thread_id, "role": "assistant", "agent": "codex", "time": { "created": created, "completed": turn.get("completedAt").and_then(Value::as_i64).unwrap_or(0) * 1000 } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": item_id, "type": "text", "text": item.get("text").and_then(Value::as_str).unwrap_or(""), "state": { "status": status } }] }));
                }
                _ => {
                    let Some(tool_part) = codex_tool_part(item_type, item) else {
                        continue;
                    };
                    let status = if tool_part.error.is_some() {
                        "error"
                    } else {
                        "completed"
                    };
                    messages.push(json!({ "info": { "id": format!("turn_{turn_id}_{item_id}"), "messageID": format!("turn_{turn_id}_{item_id}"), "sessionID": thread_id, "role": "assistant", "agent": "codex", "time": { "created": created } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": format!("turn_{turn_id}_{item_id}"), "type": "tool", "tool": tool_part.tool, "callID": item_id, "state": { "status": status, "input": tool_part.input, "output": tool_part.output.map(Value::String).unwrap_or(Value::Null), "error": tool_part.error.map(Value::String).unwrap_or(Value::Null) } }] }));
                }
            }
        }
    }
    Value::Array(messages)
}

impl Runner for CodexRunner {
    fn kind(&self) -> RunnerKind {
        RunnerKind::Codex
    }
    fn event_receiver(&self) -> Option<broadcast::Receiver<String>> {
        Some(self.runtime.events.subscribe())
    }

    fn status<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let sessions = self.runtime.sessions.read().await;
            let mut result = serde_json::Map::new();
            for (id, session) in sessions.iter() {
                if session.active_turn.is_some() {
                    result.insert(id.clone(), json!({ "type": "busy" }));
                }
            }
            Ok(Value::Object(result))
        })
    }

    fn create_session<'a>(
        &'a self,
        directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            let connection = self.connection().await?;
            let response = connection
                .request(
                    "thread/start",
                    json!({
                        "cwd": directory,
                        "approvalPolicy": "on-request",
                        "sandbox": "workspace-write",
                        "config": codex_config(&self.runtime.mcp, directory, None),
                    }),
                )
                .await?;
            let thread = response
                .get("thread")
                .context("codex did not return a thread")?;
            let id = thread
                .get("id")
                .and_then(Value::as_str)
                .context("codex did not return a thread id")?
                .to_string();
            // `thread/start` necessarily precedes the id allocation.  Refresh
            // the thread-scoped MCP config once the id exists so the manager
            // bridge can resolve `parent` for Codex-created sessions too.
            if self.runtime.mcp.current().binds_session(&RunnerKind::Codex) {
                let _ = connection
                    .request(
                        "thread/resume",
                        json!({
                            "threadId": id.clone(),
                            "cwd": directory,
                            "config": codex_config(&self.runtime.mcp, directory, Some(&id)),
                        }),
                    )
                    .await;
            }
            if !title.is_empty() {
                let _ = connection
                    .request(
                        "thread/name/set",
                        json!({ "threadId": id.clone(), "name": title }),
                    )
                    .await;
            }
            self.runtime.sessions.write().await.insert(
                id.clone(),
                CodexSessionState {
                    title: title.to_string(),
                    directory: directory.to_string(),
                    active_turn: None,
                    model: None,
                },
            );
            Ok(RunnerSession {
                id,
                title: title.to_string(),
                directory: directory.to_string(),
            })
        })
    }

    fn sessions<'a>(
        &'a self,
        directory: &'a str,
    ) -> RunnerFuture<'a, Vec<crate::app::SessionInfo>> {
        Box::pin(async move {
            let response = self.connection().await?.request(
                "thread/list",
                json!({ "cwd": directory, "limit": 200, "sortKey": "updated_at", "sortDirection": "desc" }),
            ).await?;
            Ok(response
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(codex_session_info)
                .filter(|session| session.directory == directory)
                .collect())
        })
    }

    fn messages<'a>(&'a self, session_id: &'a str, directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let connection = self.connection().await?;
            self.resume_thread(&connection, session_id, directory)
                .await?;
            let response = connection
                .request(
                    "thread/read",
                    json!({ "threadId": session_id, "includeTurns": true }),
                )
                .await?;
            let thread = response
                .get("thread")
                .context("codex did not return the thread history")?;
            let mut messages = codex_messages(thread)
                .as_array()
                .cloned()
                .unwrap_or_default();
            let rollout_history = tokio::task::spawn_blocking({
                let session_id = session_id.to_string();
                move || codex_history::load(&session_id)
            })
            .await
            .context("Codex rollout history task failed")?;
            codex_history::annotate_native_messages(&mut messages, rollout_history.message_times);
            messages.extend(rollout_history.bash_messages);
            messages = dedupe_codex_messages(messages);
            messages.sort_by_key(|message| {
                message
                    .pointer("/info/time/created")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
            });
            Ok(Value::Array(messages))
        })
    }

    fn providers<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let response = self
                .connection()
                .await?
                .request(
                    "model/list",
                    json!({ "includeHidden": false, "limit": 200 }),
                )
                .await?;
            let mut models = serde_json::Map::new();
            let mut default_model = None;
            for model in response
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = model.get("id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() || model.get("hidden").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if model.get("isDefault").and_then(Value::as_bool) == Some(true) {
                    default_model = Some(id.to_string());
                }
                models.insert(id.to_string(), json!({
                    "id": id,
                    "name": model.get("displayName").and_then(Value::as_str).unwrap_or(id),
                    "description": model.get("description").cloned().unwrap_or(Value::Null),
                    "features": model.get("inputModalities").cloned().unwrap_or_else(|| json!(["text", "image"])),
                    "reasoningEfforts": model.get("supportedReasoningEfforts").cloned().unwrap_or_else(|| json!([])),
                }));
            }
            let mut defaults = serde_json::Map::new();
            if let Some(model) = default_model {
                defaults.insert("openai".to_string(), Value::String(model));
            }
            Ok(json!({
                "all": [{ "id": "openai", "name": "OpenAI / Codex", "models": models }],
                "connected": ["openai"],
                "default": defaults,
            }))
        })
    }

    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            let connection = self.connection().await?;
            self.resume_thread(&connection, session_id, directory)
                .await?;
            let model = body
                .pointer("/model/modelID")
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut params =
                json!({ "threadId": session_id, "input": codex_input(&body), "cwd": directory });
            if let Some(model) = model.clone() {
                params["model"] = Value::String(model.clone());
            }
            if let Some(effort) = body.get("effort").and_then(Value::as_str) {
                params["effort"] = Value::String(effort.to_string());
            }
            if let Some(permission) = body.get("permission").and_then(Value::as_str) {
                match permission {
                    "never" | "on-request" | "on-failure" | "untrusted" => {
                        params["approvalPolicy"] = Value::String(permission.to_string());
                        if permission == "never" {
                            params["sandboxPolicy"] = json!({ "type": "dangerFullAccess" });
                        }
                    }
                    _ => {}
                }
            }
            let response = connection.request("turn/start", params).await?;
            let turn_id = response
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let mut sessions = self.runtime.sessions.write().await;
            let session = sessions
                .entry(session_id.to_string())
                .or_insert(CodexSessionState {
                    title: "Codex session".to_string(),
                    directory: directory.to_string(),
                    active_turn: None,
                    model: None,
                });
            session.active_turn = Some(turn_id.clone());
            session.model = model;
            Ok(json!({ "ok": true, "turn_id": turn_id }))
        })
    }

    fn abort<'a>(&'a self, session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async move {
            let turn_id = self
                .runtime
                .sessions
                .read()
                .await
                .get(session_id)
                .and_then(|session| session.active_turn.clone());
            if let Some(turn_id) = turn_id {
                self.connection()
                    .await?
                    .request(
                        "turn/interrupt",
                        json!({ "threadId": session_id, "turnId": turn_id }),
                    )
                    .await?;
            }
            Ok(())
        })
    }

    fn reply_permission<'a>(
        &'a self,
        request_id: &'a str,
        reply: &'a str,
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            let Some(request) = self
                .runtime
                .pending_requests
                .lock()
                .await
                .remove(request_id)
            else {
                return Ok(false);
            };
            let decision = match reply {
                "once" => "accept",
                "always" => "acceptForSession",
                _ => "decline",
            };
            self.connection()
                .await?
                .respond(request.rpc_id, json!({ "decision": decision }))
                .await?;
            self.runtime.emit(json!({ "type": "permission.replied", "properties": { "requestID": request_id } })).await;
            Ok(true)
        })
    }

    fn reply_question<'a>(
        &'a self,
        request_id: &'a str,
        answers: &'a [Vec<String>],
    ) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            let Some(request) = self
                .runtime
                .pending_requests
                .lock()
                .await
                .remove(request_id)
            else {
                return Ok(false);
            };
            let question_ids: Vec<String> = request
                .params
                .get("questions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|question| {
                    question
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect();
            let mapped = question_ids
                .into_iter()
                .zip(answers.iter())
                .map(|(id, answer)| (id, json!({ "answers": answer })))
                .collect::<serde_json::Map<_, _>>();
            self.connection()
                .await?
                .respond(request.rpc_id, json!({ "answers": mapped }))
                .await?;
            self.runtime.emit(json!({ "type": "question.replied", "properties": { "requestID": request_id } })).await;
            Ok(true)
        })
    }

    fn rename<'a>(&'a self, session_id: &'a str, title: &'a str) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            self.connection()
                .await?
                .request(
                    "thread/name/set",
                    json!({ "threadId": session_id, "name": title }),
                )
                .await?;
            if let Some(session) = self.runtime.sessions.write().await.get_mut(session_id) {
                session.title = title.to_string();
            }
            self.runtime.emit(json!({ "type": "session.updated", "properties": { "info": { "id": session_id, "title": title } } })).await;
            Ok(true)
        })
    }

    fn delete<'a>(&'a self, session_id: &'a str) -> RunnerFuture<'a, bool> {
        Box::pin(async move {
            self.connection()
                .await?
                .request("thread/delete", json!({ "threadId": session_id }))
                .await?;
            self.runtime.sessions.write().await.remove(session_id);
            self.runtime
                .emit(
                    json!({ "type": "session.deleted", "properties": { "sessionID": session_id } }),
                )
                .await;
            Ok(true)
        })
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

/// Routes logical sessions to runner-native sessions and performs handoffs.
pub struct RunnerRegistry {
    default: RunnerKind,
    runners: HashMap<RunnerKind, Arc<dyn Runner>>,
    bindings: RwLock<HashMap<String, Binding>>,
}

impl RunnerRegistry {
    pub fn new(default: RunnerKind, runners: HashMap<RunnerKind, Arc<dyn Runner>>) -> Self {
        Self {
            default,
            runners,
            bindings: RwLock::new(HashMap::new()),
        }
    }

    pub fn default_kind(&self) -> RunnerKind {
        self.default.clone()
    }
    pub fn available(&self) -> Vec<RunnerKind> {
        let mut runners: Vec<_> = self.runners.keys().cloned().collect();
        runners.sort_by(|a, b| a.display_name().cmp(&b.display_name()));
        runners
    }

    pub fn event_endpoints(&self) -> Vec<(RunnerKind, String)> {
        self.runners
            .iter()
            .filter_map(|(kind, runner)| runner.event_url().map(|url| (kind.clone(), url)))
            .collect()
    }

    pub fn event_receivers(&self) -> Vec<(RunnerKind, broadcast::Receiver<String>)> {
        self.runners
            .iter()
            .filter_map(|(kind, runner)| {
                runner
                    .event_receiver()
                    .map(|receiver| (kind.clone(), receiver))
            })
            .collect()
    }

    pub async fn has_binding(&self, session_id: &str) -> bool {
        self.bindings.read().await.contains_key(session_id)
    }

    pub async fn has_or_bind_known_session(&self, session_id: &str, directory: &str) -> bool {
        if validate_location(session_id, directory).is_err() {
            return false;
        }
        if self.has_binding(session_id).await {
            return true;
        }
        if is_codex_thread_id(session_id) {
            let _ = self.binding(session_id, directory).await;
            return self.has_binding(session_id).await;
        }
        false
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
        if is_codex_thread_id(session_id) {
            if let Some(codex) = self.runners.get(&RunnerKind::Codex).cloned() {
                if codex.messages(session_id, directory).await.is_ok() {
                    let binding = Binding {
                        runner: RunnerKind::Codex,
                        physical_id: session_id.to_string(),
                        directory: directory.to_string(),
                    };
                    self.bindings
                        .write()
                        .await
                        .insert(session_id.to_string(), binding.clone());
                    return binding;
                }
            }
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
        self.runners
            .get(&binding.runner)
            .context("runner is not available")?
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
        let runner = self
            .runners
            .get(&binding.runner)
            .context("runner is not available")?;
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
        let probes = self.runners.iter().map(|(kind, runner)| async move {
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
        for (kind, runner) in &self.runners {
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
        let runner = self.runners.get(&kind).context("runner is not available")?;
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

    pub async fn providers(&self, kind: RunnerKind, directory: &str) -> Result<Value> {
        let directory = ProjectDirectory::new(directory)
            .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        self.runners
            .get(&kind)
            .context("runner is not available")?
            .providers(directory)
            .await
    }

    pub async fn agents(&self, kind: RunnerKind, directory: &str) -> Result<Value> {
        let directory = ProjectDirectory::new(directory)
            .map_err(|error| anyhow::anyhow!("invalid project directory: {error}"))?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        self.runners
            .get(&kind)
            .context("runner is not available")?
            .agents(directory)
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
            .runners
            .get(&target_kind)
            .context("requested runner is not available")?
            .clone();
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
            .runners
            .get(&current.runner)
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
        self.runners
            .get(&binding.runner)
            .context("runner is not available")?
            .abort(&binding.physical_id, &binding.directory)
            .await
    }

    pub async fn rename(&self, session_id: &str, title: &str, directory: &str) -> Result<bool> {
        let (session_id, directory) = validate_location(session_id, directory)?;
        let directory = directory
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("project directory is not valid UTF-8"))?;
        let binding = self.binding(session_id.as_str(), directory).await;
        self.runners
            .get(&binding.runner)
            .context("runner is not available")?
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
            .runners
            .get(&binding.runner)
            .context("runner is not available")?
            .delete(&binding.physical_id)
            .await?;
        if deleted {
            self.bindings.write().await.remove(session_id.as_str());
        }
        Ok(deleted)
    }

    pub async fn reply_permission(&self, request_id: &str, reply: &str) -> Result<bool> {
        for runner in self.runners.values() {
            if runner.reply_permission(request_id, reply).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn reply_question(&self, request_id: &str, answers: &[Vec<String>]) -> Result<bool> {
        for runner in self.runners.values() {
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

fn is_codex_thread_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
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
        for kind in ["busy", "retry", "working", "active"] {
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
        assert_eq!(RunnerKind::parse("codex"), Some(RunnerKind::Codex));
        assert_eq!(RunnerKind::parse("nope"), None);
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

    #[test]
    fn codex_thread_ids_are_detectable_without_prefixes() {
        assert!(is_codex_thread_id("019fc856-21ec-7fd2-bfdd-ac3ba506da18"));
        assert!(!is_codex_thread_id("ses_opencode_session"));
    }

    /// Built from an explicit spec set rather than the real built-ins, so the
    /// assertions do not depend on whether this machine happens to have the web
    /// server's descriptor on disk or the agent-manager socket in its environment.
    #[test]
    fn codex_mcp_config_is_thread_scoped_and_project_bound() {
        use crate::mcp_registry::spec::{Arg, ServerSpec};
        use crate::mcp_registry::{BuiltinFlags, McpRegistry, RegistryHandle};

        let registry = RegistryHandle::new(
            Arc::new(McpRegistry::from_specs(
                vec![
                ServerSpec::stdio(
                    "terminal",
                    "/opman",
                    vec![Arg::lit("mcp"), Arg::Dir],
                    vec![("OPENCODE_SESSION_ID".into(), Arg::SessionId)],
                ),
                ServerSpec::stdio(
                    "neovim",
                    "/opman",
                    vec![Arg::lit("mcp-nvim"), Arg::Dir],
                    Vec::new(),
                ),
                ServerSpec::stdio("time", "/opman", vec![Arg::lit("mcp-time")], Vec::new()),
                ],
                BuiltinFlags::default(),
            )),
            BuiltinFlags::default(),
        );

        let payload = codex_config(&registry, "/workspace/project", Some("session"));
        let servers = payload
            .get("mcp_servers")
            .and_then(Value::as_object)
            .expect("MCP server map should be present");
        assert_eq!(servers.len(), 3);
        assert_eq!(servers["terminal"]["args"][0], "mcp");
        assert_eq!(servers["terminal"]["args"][1], "/workspace/project");
        assert_eq!(servers["terminal"]["env"]["OPENCODE_SESSION_ID"], "session");
        assert_eq!(servers["neovim"]["args"][1], "/workspace/project");
        assert_eq!(servers["time"]["args"][0], "mcp-time");
    }

    #[test]
    fn codex_transcript_maps_native_items_to_opencode_messages(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let transcript = codex_messages(&json!({
            "id": "thread",
            "turns": [{
                "id": "turn",
                "startedAt": 10,
                "completedAt": 11,
                "items": [
                    { "type": "userMessage", "id": "user", "content": [{ "type": "text", "text": "hello" }] },
                    { "type": "agentMessage", "id": "agent", "text": "world" },
                    { "type": "commandExecution", "id": "cmd", "command": "pwd", "aggregatedOutput": "/tmp" },
                    { "type": "fileChange", "id": "edit", "changes": [] }
                ]
            }]
        }));
        let messages = transcript
            .as_array()
            .ok_or("Codex transcript was not an array")?;
        assert_eq!(messages.len(), 4);
        assert_eq!(transcript[1]["parts"][0]["text"], "world");
        assert_eq!(transcript[2]["parts"][0]["tool"], "bash");
        assert_eq!(transcript[3]["parts"][0]["tool"], "edit");
        assert_ne!(
            transcript[2]["info"]["messageID"],
            transcript[3]["info"]["messageID"]
        );
        Ok(())
    }

    #[test]
    fn codex_tool_mapper_preserves_native_tool_kinds() {
        let cases = [
            ("commandExecution", json!({ "command": "pwd" }), "bash"),
            ("fileChange", json!({ "changes": [] }), "edit"),
            (
                "mcpToolCall",
                json!({ "server": "docs", "tool": "lookup" }),
                "mcp_docs_lookup",
            ),
            (
                "dynamicToolCall",
                json!({ "namespace": "local", "name": "inspect" }),
                "dynamic_local_inspect",
            ),
            (
                "collabAgentToolCall",
                json!({ "prompt": "delegate" }),
                "task",
            ),
            (
                "subAgentActivity",
                json!({ "agentThreadId": "agent" }),
                "task",
            ),
            ("webSearch", json!({ "query": "rust" }), "web_search"),
            ("imageView", json!({ "path": "/tmp/a.png" }), "image_view"),
            ("sleep", json!({ "durationMs": 10 }), "sleep"),
            (
                "imageGeneration",
                json!({ "prompt": "icon" }),
                "image_generation",
            ),
            ("plan", json!({ "text": "step" }), "plan"),
        ];

        for (item_type, item, expected_tool) in cases {
            let mapped = codex_tool_part(item_type, &item);
            assert_eq!(
                mapped.as_ref().map(|part| part.tool.as_str()),
                Some(expected_tool)
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires a locally authenticated Codex installation"]
    async fn codex_app_server_can_create_and_read_a_thread(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let runner = CodexRunner::new(
            reqwest::Client::new(),
            crate::mcp_registry::RegistryHandle::default(),
        );
        let session = runner.create_session("/tmp", "OpMan test").await?;
        let messages = runner.messages(&session.id, "/tmp").await?;
        assert!(messages.as_array().is_some());
        Ok(())
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
            directory: &'a str,
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
                    directory: directory.to_string(),
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
