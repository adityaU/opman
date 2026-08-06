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

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
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

/// A live JSON-RPC connection. Cloneable: every clone talks to the same child.
#[derive(Clone)]
pub struct Peer {
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    waiters: Waiters,
    next_id: Arc<AtomicI64>,
}

impl Peer {
    /// Wrap a child's pipes and start the reader task. The reader ends at EOF, failing
    /// every outstanding request so no caller waits on a dead process forever.
    pub fn new<H: Handler>(stdin: ChildStdin, stdout: ChildStdout, handler: Arc<H>) -> Self {
        let peer = Self {
            stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
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

async fn read_loop<H: Handler>(peer: Peer, stdout: ChildStdout, handler: Arc<H>) {
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

/// Split a JSON-RPC response into `Ok(result)` / `Err(error.message)`.
fn response_outcome(frame: &Value) -> Result<Value> {
    if let Some(err) = frame.get("error") {
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        let details = err
            .get("data")
            .and_then(|d| d.get("details"))
            .and_then(Value::as_str);
        bail!(match details {
            Some(d) => format!("{message}: {d}"),
            None => message.to_string(),
        });
    }
    Ok(frame.get("result").cloned().unwrap_or(json!({})))
}

#[cfg(test)]
#[path = "jsonrpc_tests.rs"]
mod jsonrpc_tests;
