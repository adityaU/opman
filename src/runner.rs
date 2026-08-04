//! Common runner abstraction used by the web backend.
//!
//! A session is deliberately kept separate from a runner.  This is what lets a
//! client select a different runner for the next turn: the registry creates a
//! new runner-native session, sends a handoff context to it, and returns the
//! new session id to the client.

use std::collections::HashMap;
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
pub use opman_backend_contracts::{ProjectDirectory, RunnerKind, SessionId};

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
    fn providers<'a>(&'a self, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async { Ok(json!({ "all": [], "connected": [], "default": {} })) })
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
                    if matches!(object.get("agent").and_then(Value::as_str), Some("default") | Some("claude") | Some("codex")) {
                        object.remove("agent");
                    }
                }
            }
            self.json_request(
                self.client
                    .post(format!("{}/session/{}/message", self.base_url, session_id))
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
}

/// JSON-RPC transport for Codex's native app-server protocol.
struct CodexConnection {
    _child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>,
    next_id: AtomicU64,
    events: broadcast::Sender<Value>,
}

/// OpMan MCPs made available to a Codex thread.
#[derive(Clone, Debug, Default)]
pub struct CodexMcpConfig {
    pub terminal: bool,
    pub neovim: bool,
    pub time: bool,
    pub ui: bool,
    pub kanban: bool,
}

impl CodexMcpConfig {
    fn for_directory(&self, directory: &str) -> Value {
        let executable = std::env::current_exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "opman".to_string());
        let mut servers = serde_json::Map::new();

        if self.terminal {
            servers.insert(
                "terminal".to_string(),
                json!({ "command": executable, "args": ["mcp", directory] }),
            );
        }
        if self.neovim {
            servers.insert(
                "neovim".to_string(),
                json!({ "command": executable, "args": ["mcp-nvim", directory] }),
            );
        }
        if self.time {
            servers.insert(
                "time".to_string(),
                json!({ "command": executable, "args": ["mcp-time"] }),
            );
        }
        if self.ui {
            servers.insert(
                "ui".to_string(),
                json!({ "command": executable, "args": ["mcp-ui"] }),
            );
        }
        if self.kanban {
            servers.insert(
                "kanban".to_string(),
                json!({ "command": executable, "args": ["mcp-kanban"] }),
            );
        }

        json!({ "mcp_servers": servers })
    }
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
    mcp: CodexMcpConfig,
    connection: OnceCell<Arc<CodexConnection>>,
    sessions: RwLock<HashMap<String, CodexSessionState>>,
    pending_requests: Mutex<HashMap<String, CodexPendingRequest>>,
    tool_outputs: Mutex<HashMap<String, String>>,
    events: broadcast::Sender<String>,
    event_task_started: AtomicU64,
}

/// Full Codex app-server adapter. It keeps one long-lived JSON-RPC process,
/// persists threads through Codex's own rollout store, and translates native
/// notifications into the OpenCode-shaped event stream consumed by OpMan.
pub struct CodexRunner {
    runtime: Arc<CodexRuntime>,
}

impl CodexRunner {
    pub fn new(_client: reqwest::Client, mcp: CodexMcpConfig) -> Self {
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
                    "config": self.runtime.mcp.for_directory(directory),
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
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => self.emit_reasoning_delta(&params).await,
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
                let text = item.get("text").and_then(Value::as_str).or_else(|| item.get("summary").and_then(Value::as_str)).unwrap_or("");
                let message_id = format!("turn_{}", string_at(params, "turnId"));
                let info = json!({ "id": message_id, "messageID": message_id, "sessionID": session_id, "role": "assistant", "time": { "created": timestamp } });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } })).await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "reasoning", "text": text } } })).await;
            }
            "agentMessage" => {
                let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                let info = json!({ "id": item_id, "messageID": item_id, "sessionID": session_id, "role": "assistant", "time": { "created": timestamp }, "modelID": self.sessions.read().await.get(&session_id).and_then(|s| s.model.clone()) });
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
                let info = json!({ "id": message_id, "messageID": message_id, "sessionID": session_id, "role": "assistant", "time": { "created": timestamp } });
                self.emit(json!({ "type": "message.updated", "properties": { "info": info } }))
                    .await;
                self.emit(json!({ "type": "message.part.updated", "properties": { "part": { "id": item_id, "sessionID": session_id, "messageID": message_id, "type": "tool", "tool": tool_part.tool, "callID": item_id, "state": state } } })).await;
            }
        }
    }

    async fn emit_reasoning_delta(&self, params: &Value) {
        let Some(delta) = params.get("delta").and_then(Value::as_str).or_else(|| params.get("text").and_then(Value::as_str)) else { return; };
        if delta.is_empty() { return; }
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
        self.emit(json!({ "type": "message.updated", "properties": { "info": { "id": format!("usage_{}", string_at(params, "turnId")), "messageID": format!("usage_{}", string_at(params, "turnId")), "sessionID": session_id, "role": "assistant", "tokens": { "input": usage.get("inputTokens").cloned().unwrap_or(json!(0)), "output": usage.get("outputTokens").cloned().unwrap_or(json!(0)), "reasoning": usage.get("reasoningOutputTokens").cloned().unwrap_or(json!(0)), "cache": { "read": usage.get("cachedInputTokens").cloned().unwrap_or(json!(0)) } } } } })).await;
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
                    let text = item.get("text").and_then(Value::as_str).or_else(|| item.get("summary").and_then(Value::as_str)).unwrap_or("");
                    messages.push(json!({ "info": { "id": item_id, "messageID": item_id, "sessionID": thread_id, "role": "assistant", "time": { "created": created } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": item_id, "type": "reasoning", "text": text }] }));
                }
                "agentMessage" => {
                    let status = if turn.get("status").and_then(Value::as_str) == Some("failed") {
                        "error"
                    } else {
                        "completed"
                    };
                    messages.push(json!({ "info": { "id": item_id, "messageID": item_id, "sessionID": thread_id, "role": "assistant", "time": { "created": created, "completed": turn.get("completedAt").and_then(Value::as_i64).unwrap_or(0) * 1000 } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": item_id, "type": "text", "text": item.get("text").and_then(Value::as_str).unwrap_or(""), "state": { "status": status } }] }));
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
                    messages.push(json!({ "info": { "id": format!("turn_{turn_id}_{item_id}"), "messageID": format!("turn_{turn_id}_{item_id}"), "sessionID": thread_id, "role": "assistant", "time": { "created": created } }, "parts": [{ "id": item_id, "sessionID": thread_id, "messageID": format!("turn_{turn_id}_{item_id}"), "type": "tool", "tool": tool_part.tool, "callID": item_id, "state": { "status": status, "input": tool_part.input, "output": tool_part.output.map(Value::String).unwrap_or(Value::Null), "error": tool_part.error.map(Value::String).unwrap_or(Value::Null) } }] }));
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
                        "config": self.runtime.mcp.for_directory(directory),
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
        runners.sort_by_key(|runner| runner.display_name());
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
        let summary = summarize_transcript(&history);
        let user_text = extract_text(&body);
        let handoff = format!(
            "You are taking over a coding session from the {} runner.\n\nSession summary:\n{}\n\nContinue with this new user request:\n{}",
            current.runner.display_name(), summary, user_text
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

/// Produce a bounded, deterministic handoff summary. It deliberately does not
/// call an LLM, so switching runners works even when the old runner is offline.
pub fn summarize_transcript(body: &Value) -> String {
    let values: Vec<Value> = if let Some(array) = body.as_array() {
        array.clone()
    } else if let Some(object) = body.as_object() {
        object.values().cloned().collect()
    } else {
        Vec::new()
    };
    let mut lines = Vec::new();
    for message in values {
        let role = message
            .pointer("/info/role")
            .and_then(Value::as_str)
            .unwrap_or("message");
        let text = message
            .get("parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        if !text.trim().is_empty() {
            lines.push(format!("{}: {}", role, text.trim()));
        }
    }
    let mut result = lines.join("\n");
    const MAX: usize = 12_000;
    if result.len() > MAX {
        result.truncate(MAX);
        result.push_str("\n[Earlier transcript omitted]");
    }
    if result.is_empty() {
        "No transcript was available.".to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn parses_runner_names() {
        assert_eq!(RunnerKind::parse("claude-code"), Some(RunnerKind::Claude));
        assert_eq!(RunnerKind::parse("codex"), Some(RunnerKind::Codex));
        assert_eq!(RunnerKind::parse("nope"), None);
    }

    #[test]
    fn transcript_summary_is_bounded_and_readable() {
        let body = json!([{ "info": { "role": "user" }, "parts": [{ "text": "hello" }] }]);
        assert_eq!(summarize_transcript(&body), "user: hello");
    }

    #[test]
    fn codex_thread_ids_are_detectable_without_prefixes() {
        assert!(is_codex_thread_id("019fc856-21ec-7fd2-bfdd-ac3ba506da18"));
        assert!(!is_codex_thread_id("ses_opencode_session"));
    }

    #[test]
    fn codex_mcp_config_is_thread_scoped_and_project_bound() {
        let config = CodexMcpConfig {
            terminal: true,
            neovim: true,
            time: true,
            ui: true,
            kanban: true,
        };
        let payload = config.for_directory("/workspace/project");
        let servers = payload
            .get("mcp_servers")
            .and_then(Value::as_object)
            .expect("MCP server map should be present");
        assert_eq!(servers.len(), 5);
        assert_eq!(servers["terminal"]["args"][0], "mcp");
        assert_eq!(servers["terminal"]["args"][1], "/workspace/project");
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
        let runner = CodexRunner::new(reqwest::Client::new(), CodexMcpConfig::default());
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
            kind: RunnerKind::Claude,
            prefix: "new",
            next: AtomicUsize::new(1),
            sessions: RwLock::new(HashMap::new()),
        });
        let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
        runners.insert(RunnerKind::Opencode, old.clone());
        runners.insert(RunnerKind::Claude, new.clone());
        let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);
        let outcome = registry
            .send_message(
                "logical",
                "/project",
                Some(RunnerKind::Claude),
                json!({
                    "parts": [{ "type": "text", "text": "Now add a regression test" }]
                }),
            )
            .await?;
        assert!(outcome.switched);
        assert_eq!(outcome.runner, RunnerKind::Claude);
        assert!(outcome.session_id.starts_with("new_"));
        let handoff = outcome.response["parts"][0]["text"]
            .as_str()
            .ok_or("handoff response did not contain text")?;
        assert!(handoff.contains("Fix the parser"));
        Ok(())
    }
}
