//! JSON-RPC peer for a language server.
//!
//! The mechanics — atomic request ids, a waiter registered *before* the write,
//! dispatch by frame shape, inbound requests answered on their own task, and
//! every waiter failed at EOF — are the ones proven in
//! [`crate::acp_engine::jsonrpc`]. Three things differ, each for a reason:
//!
//! * frames are [`super::framing`]-encoded, not newline-delimited;
//! * writes go through an mpsc channel to a single writer task rather than a
//!   mutex over stdin, because ordering is a correctness requirement here —
//!   `didOpen` must reach the server before the `hover` that depends on it, and
//!   two tasks awaiting a mutex have no defined order;
//! * requests carry a timeout and cancel themselves, because a wedged server
//!   must not pin an axum handler forever.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

use super::framing::{read_frame, write_frame, Transport};

/// Handles frames the peer receives rather than sends.
pub trait Handler: Send + Sync + 'static {
    /// Answer a server→client request. Servers block on some of these
    /// (`workspace/configuration` during startup), so answering is mandatory.
    fn request(&self, method: &str, params: &Value) -> Result<Value>;
    /// Consume a server→client notification. Errors have nowhere to go.
    fn notify(&self, method: &str, params: Value);
}

type Waiters = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>;

/// A live connection to one language server. Cloning is cheap; every clone
/// talks to the same child.
#[derive(Clone)]
pub struct Peer {
    outbox: mpsc::UnboundedSender<Value>,
    waiters: Waiters,
    next_id: Arc<AtomicI64>,
    alive: Arc<AtomicBool>,
}

impl Peer {
    /// Wrap a transport and start the reader and writer tasks.
    pub fn new<T: Transport, H: Handler>(transport: T, handler: Arc<H>) -> Self {
        let (reader, writer) = transport.split();
        let (outbox, inbox) = mpsc::unbounded_channel();

        let peer = Self {
            outbox,
            waiters: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicI64::new(1)),
            alive: Arc::new(AtomicBool::new(true)),
        };

        tokio::spawn(write_loop(writer, inbox));
        tokio::spawn(read_loop(peer.clone(), reader, handler));
        peer
    }

    /// True until the server's stdout reaches EOF.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Send a request and await its response, giving up after `timeout`.
    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        if !self.is_alive() {
            bail!("language server is not running");
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .map_err(|_| anyhow!("lsp waiter registry poisoned"))?
            .insert(id, tx);

        if let Err(e) = self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        })) {
            self.forget(id);
            return Err(e);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => bail!("language server closed while awaiting `{method}`"),
            Err(_) => {
                self.forget(id);
                // Tell the server to stop working on it; a dropped waiter would
                // otherwise leave it burning CPU on an answer nobody wants.
                let _ = self.notify("$/cancelRequest", json!({ "id": id }));
                bail!("`{method}` timed out after {:?}", timeout)
            }
        }
    }

    /// Send a notification. Synchronous and non-blocking, so bookkeeping under
    /// a lock can emit one without an await point.
    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    fn send(&self, frame: Value) -> Result<()> {
        self.outbox
            .send(frame)
            .map_err(|_| anyhow!("language server connection is closed"))
    }

    fn forget(&self, id: i64) {
        self.waiters.lock().ok().and_then(|mut w| w.remove(&id));
    }

    fn resolve(&self, id: i64, outcome: Result<Value>) {
        let tx = self.waiters.lock().ok().and_then(|mut w| w.remove(&id));
        if let Some(tx) = tx {
            let _ = tx.send(outcome);
        }
    }

    /// Fail every outstanding request once the server is gone.
    fn abandon(&self, reason: &str) {
        self.alive.store(false, Ordering::Relaxed);
        let Ok(mut waiters) = self.waiters.lock() else {
            return;
        };
        for (_, tx) in waiters.drain() {
            let _ = tx.send(Err(anyhow!("{reason}")));
        }
    }
}

async fn write_loop<W>(mut writer: W, mut inbox: mpsc::UnboundedReceiver<Value>)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    while let Some(frame) = inbox.recv().await {
        if let Err(e) = write_frame(&mut writer, &frame).await {
            debug!("lsp: write failed, stopping writer: {e}");
            return;
        }
    }
}

async fn read_loop<R, H>(peer: Peer, reader: R, handler: Arc<H>)
where
    R: tokio::io::AsyncRead + Unpin,
    H: Handler,
{
    let mut reader = tokio::io::BufReader::new(reader);
    loop {
        match read_frame(&mut reader).await {
            Ok(Some(frame)) => dispatch(&peer, &handler, frame),
            Ok(None) => break,
            Err(e) => {
                debug!("lsp: framing error, dropping connection: {e}");
                peer.abandon(&format!("language server stream error: {e}"));
                return;
            }
        }
    }
    peer.abandon("language server exited");
}

/// Route one frame. A frame with a `method` is inbound work; one without is a
/// response to something we sent. Inbound requests carry both, so `method` wins.
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

    let Some(id) = id else {
        handler.notify(&method, params);
        return;
    };

    let reply = match handler.request(&method, &params) {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(e) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": e.to_string() },
        }),
    };
    if peer.send(reply).is_err() {
        debug!(%method, "lsp: could not answer inbound request");
    }
}

/// Split a JSON-RPC response into `Ok(result)` / `Err(error.message)`.
fn response_outcome(frame: &Value) -> Result<Value> {
    if let Some(err) = frame.get("error") {
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("{message}");
    }
    Ok(frame.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
#[path = "peer_tests.rs"]
mod peer_tests;
