//! Bidirectional newline-delimited JSON-RPC 2.0 over a child process's stdio.
//!
//! ACP is symmetric: while opman is awaiting `session/prompt`, the agent sends its own
//! requests back (`session/request_permission`, `fs/*`, `terminal/*`) on the same pipe.
//! A response-only client would deadlock — the agent blocks on a permission answer that
//! never comes because opman is blocked on the prompt. So this peer runs one reader task
//! that routes each frame by shape: responses wake the matching waiter, requests and
//! notifications go to a [`Handler`] spawned per frame.
//!
//! Nothing here knows what ACP is; [`super::conn`] supplies the meaning.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use tracing::debug;

/// Handles frames the peer receives rather than sends: agent→client requests (which
/// must produce a result) and notifications (which must not).
pub trait Handler: Send + Sync + 'static {
    /// Answer an agent→client request. `Err` is returned to the agent as a JSON-RPC error.
    fn request(
        self: Arc<Self>,
        method: String,
        params: Value,
    ) -> futures::future::BoxFuture<'static, Result<Value>>;

    /// Consume an agent→client notification. Errors have nowhere to go, so there is no
    /// return value; log instead.
    fn notify(self: Arc<Self>, method: String, params: Value);
}

type Waiters = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>;

/// The write half of a peer. Boxed rather than generic so [`Peer`] stays one concrete type
/// for everything holding it, and so a test can drive one over an in-memory pipe instead of
/// a spawned process. One dynamic call per request is not a cost worth a generic parameter
/// threaded through the whole connection.
type Writer = Box<dyn AsyncWrite + Send + Unpin>;

/// A live JSON-RPC connection. Cloneable: every clone talks to the same child.
#[derive(Clone)]
pub struct Peer {
    stdin: Arc<tokio::sync::Mutex<Writer>>,
    waiters: Waiters,
    next_id: Arc<AtomicI64>,
}

impl Peer {
    /// Wrap a child's pipes and start the reader task. The reader ends at EOF, failing
    /// every outstanding request so no caller waits on a dead process forever.
    pub fn new<H: Handler>(
        stdin: impl AsyncWrite + Send + Unpin + 'static,
        stdout: impl AsyncRead + Send + Unpin + 'static,
        handler: Arc<H>,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(tokio::sync::Mutex::new(Box::new(stdin) as Writer)),
            waiters: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicI64::new(1)),
        };
        tokio::spawn(read_loop(peer.clone(), stdout, handler));
        peer
    }

    /// Send a request and await its response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .map_err(|_| anyhow!("acp waiter registry poisoned"))?
            .insert(id, tx);
        if let Err(e) = self
            .write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await
        {
            self.waiters.lock().ok().and_then(|mut w| w.remove(&id));
            return Err(e);
        }
        rx.await
            .map_err(|_| anyhow!("acp connection closed while awaiting `{method}`"))?
    }

    /// Send a notification (no response expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
            .await
    }

    async fn write(&self, frame: &Value) -> Result<()> {
        let mut line = serde_json::to_vec(frame)?;
        line.push(b'\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(&line).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Fail every outstanding request. Called once the child's stdout reaches EOF.
    fn abandon_waiters(&self, reason: &str) {
        let Ok(mut w) = self.waiters.lock() else {
            return;
        };
        for (_, tx) in w.drain() {
            let _ = tx.send(Err(anyhow!("{reason}")));
        }
    }

    fn resolve(&self, id: i64, outcome: Result<Value>) {
        let tx = self.waiters.lock().ok().and_then(|mut w| w.remove(&id));
        if let Some(tx) = tx {
            let _ = tx.send(outcome);
        }
    }
}

async fn read_loop<H: Handler>(peer: Peer, stdout: impl AsyncRead + Send + Unpin, handler: Arc<H>) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            // A non-JSON line is an agent writing diagnostics to stdout; skip it rather
            // than tearing down an otherwise healthy connection.
            Err(_) => debug!(%line, "acp: ignoring non-JSON frame"),
            Ok(frame) => dispatch(&peer, &handler, frame),
        }
    }
    peer.abandon_waiters("acp process exited");
}

/// Route one decoded frame. A frame with a `method` is inbound work; one without is a
/// response to something we sent. An inbound request carries both, so `method` is
/// checked first.
fn dispatch<H: Handler>(peer: &Peer, handler: &Arc<H>, frame: Value) {
    let id = frame.get("id").and_then(Value::as_i64);
    let Some(method) = frame.get("method").and_then(Value::as_str) else {
        if let Some(id) = id {
            peer.resolve(id, response_outcome(&frame));
        }
        return;
    };
    let method = method.to_string();
    let params = frame.get("params").cloned().unwrap_or(json!({}));
    let handler = handler.clone();
    let Some(id) = id else {
        handler.notify(method, params);
        return;
    };
    // Answer on a task: handlers block on user input (permission prompts), and the read
    // loop must stay free to receive the next frame while they do.
    let peer = peer.clone();
    tokio::spawn(async move {
        let reply = match handler.request(method.clone(), params).await {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(e) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32603, "message": e.to_string() },
            }),
        };
        if let Err(e) = peer.write(&reply).await {
            debug!(%method, "acp: failed to answer inbound request: {e}");
        }
    });
}

/// ACP's "log in first" code, returned by `session/new` when the agent has no credential.
pub const AUTH_REQUIRED: i64 = -32000;

/// An error the peer sent back, with its code kept.
///
/// The code is the only thing separating "you must authenticate" from any other failure, and
/// the stringified error this replaced threw it away — which is why an agent that needed a
/// login looked exactly like an agent that was broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

/// JSON-RPC's own "no such method", which for ACP means a capability the agent published but
/// does not actually serve.
pub const METHOD_NOT_FOUND: i64 = -32601;

fn has_code(error: &anyhow::Error, code: i64) -> bool {
    error
        .downcast_ref::<RpcError>()
        .is_some_and(|rpc| rpc.code == code)
}

/// Whether a failure was the agent asking to be authenticated first.
pub fn needs_auth(error: &anyhow::Error) -> bool {
    has_code(error, AUTH_REQUIRED)
}

/// Whether the agent does not implement the method at all, as opposed to refusing the call.
pub fn unimplemented(error: &anyhow::Error) -> bool {
    has_code(error, METHOD_NOT_FOUND)
}

/// Split a JSON-RPC response into `Ok(result)` / `Err(RpcError)`.
fn response_outcome(frame: &Value) -> Result<Value> {
    let Some(err) = frame.get("error") else {
        return Ok(frame.get("result").cloned().unwrap_or(json!({})));
    };
    let message = err
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    let details = err
        .get("data")
        .and_then(|d| d.get("details"))
        .and_then(Value::as_str);
    Err(RpcError {
        code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: match details {
            Some(d) => format!("{message}: {d}"),
            None => message.to_string(),
        },
    }
    .into())
}

/// An in-process stand-in for an ACP agent.
///
/// The protocol's interesting behaviour is in the round-trip — a refusal that must be
/// retried, a method that turns out not to exist — none of which a hand-built reply value can
/// exercise. `answer` sees every request the peer sends and returns either a result or a
/// JSON-RPC error object.
#[cfg(test)]
pub(super) fn fake_agent<F>(answer: F) -> Peer
where
    F: Fn(&str, &Value) -> std::result::Result<Value, Value> + Send + 'static,
{
    struct Deaf;
    impl Handler for Deaf {
        fn request(
            self: Arc<Self>,
            method: String,
            _params: Value,
        ) -> futures::future::BoxFuture<'static, Result<Value>> {
            Box::pin(async move { Err(anyhow!("unexpected inbound `{method}`")) })
        }
        fn notify(self: Arc<Self>, _method: String, _params: Value) {}
    }

    let (to_agent, from_client) = tokio::io::duplex(64 * 1024);
    let (mut to_client, from_agent) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let mut lines = BufReader::new(from_client).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let (Some(method), Some(id)) = (
                frame.get("method").and_then(Value::as_str),
                frame.get("id").cloned(),
            ) else {
                continue;
            };
            let params = frame.get("params").cloned().unwrap_or(json!({}));
            let reply = match answer(method, &params) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
            };
            let mut out = serde_json::to_vec(&reply).unwrap_or_default();
            out.push(b'\n');
            if to_client.write_all(&out).await.is_err() {
                return;
            }
        }
    });
    Peer::new(to_agent, from_agent, Arc::new(Deaf))
}

#[cfg(test)]
#[path = "jsonrpc_tests.rs"]
mod jsonrpc_tests;
