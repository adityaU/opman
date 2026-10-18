//! Multiplexed MCP connection: independent writer, reader, and waiting callers.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;

const PROTOCOL_VERSION: &str = "2025-06-18";
/// A single message larger than this closes the connection instead of growing memory.
const MAX_MESSAGE_BYTES: u64 = 16 * 1024 * 1024;
const OUTBOX: usize = 64;

#[derive(Debug)]
pub enum CallError {
    Rpc { code: i64, message: String },
    Idle(Duration),
    Closed,
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rpc { code, message } => write!(f, "{message} (code {code})"),
            Self::Idle(window) => write!(
                f,
                "no reply or progress for {}s, so the call was given up",
                window.as_secs()
            ),
            Self::Closed => f.write_str("the server closed the connection"),
        }
    }
}

type Reply = Result<Value, CallError>;

struct Slot {
    reply: oneshot::Sender<Reply>,
    /// Poked by `notifications/progress`, which restarts the caller's idle clock.
    activity: Arc<Notify>,
}

#[derive(Default)]
struct Pending(Mutex<HashMap<u64, Slot>>);

impl Pending {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Slot>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct Connection {
    out: mpsc::Sender<Vec<u8>>,
    pending: Arc<Pending>,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
    stale: Arc<AtomicBool>,
    tasks: [JoinHandle<()>; 2],
    /// Held only so the child is killed when the connection goes away.
    _child: Option<tokio::process::Child>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Connection {
    /// Handshake over `reader`/`writer`: `initialize`, then `notifications/initialized`
    /// (a notification, so no reply is awaited).
    pub async fn open<R, W>(
        reader: R,
        writer: W,
        child: Option<tokio::process::Child>,
    ) -> Result<Self, CallError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (out, queue) = mpsc::channel(OUTBOX);
        let pending = Arc::new(Pending::default());
        let alive = Arc::new(AtomicBool::new(true));
        let stale = Arc::new(AtomicBool::new(false));
        let tasks = [
            tokio::spawn(write_loop(writer, queue)),
            tokio::spawn(read_loop(
                reader,
                pending.clone(),
                out.clone(),
                alive.clone(),
                stale.clone(),
            )),
        ];
        let conn = Self {
            out,
            pending,
            next_id: AtomicU64::new(1),
            alive,
            stale,
            tasks,
            _child: child,
        };
        let hello = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "opman-code", "version": env!("CARGO_PKG_VERSION") },
        });
        conn.request("initialize", hello, Duration::from_secs(20))
            .await?;
        conn.notify("notifications/initialized", Value::Null).await;
        Ok(conn)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// True once per `tools/list_changed`.
    pub fn take_stale(&self) -> bool {
        self.stale.swap(false, Ordering::Relaxed)
    }

    pub async fn notify(&self, method: &str, params: Value) {
        let mut message = json!({ "jsonrpc": "2.0", "method": method });
        if !params.is_null() {
            message["params"] = params;
        }
        let _ = self.out.send(encode(&message)).await;
    }

    /// Send a request and wait for its reply. `idle` is how long the server may stay
    /// silent; progress notifications for this request restart the clock. Dropping the
    /// future early tells the server the request is cancelled.
    pub async fn request(&self, method: &str, mut params: Value, idle: Duration) -> Reply {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if method == "tools/call" {
            params["_meta"] = json!({ "progressToken": id });
        }
        let (reply, mut answer) = oneshot::channel();
        let activity = Arc::new(Notify::new());
        self.pending.lock().insert(
            id,
            Slot {
                reply,
                activity: activity.clone(),
            },
        );
        let mut guard = Cancelling {
            id,
            pending: &self.pending,
            out: &self.out,
            armed: true,
        };
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if self.out.send(encode(&message)).await.is_err() {
            return Err(CallError::Closed);
        }
        loop {
            tokio::select! {
                result = &mut answer => {
                    guard.armed = false;
                    return result.unwrap_or(Err(CallError::Closed));
                }
                () = activity.notified() => {}
                () = tokio::time::sleep(idle) => return Err(CallError::Idle(idle)),
            }
        }
    }
}

/// Cleans up a request that did not finish, and tells the server to stop working on it.
struct Cancelling<'a> {
    id: u64,
    pending: &'a Pending,
    out: &'a mpsc::Sender<Vec<u8>>,
    armed: bool,
}

impl Drop for Cancelling<'_> {
    fn drop(&mut self) {
        self.pending.lock().remove(&self.id);
        if !self.armed {
            return;
        }
        let message = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": self.id, "reason": "the script stopped waiting" },
        });
        // Best effort: a full queue means the server is not reading anyway.
        let _ = self.out.try_send(encode(&message));
    }
}

fn encode(message: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

async fn write_loop<W: AsyncWrite + Unpin>(mut writer: W, mut queue: mpsc::Receiver<Vec<u8>>) {
    while let Some(bytes) = queue.recv().await {
        if writer.write_all(&bytes).await.is_err() || writer.flush().await.is_err() {
            return;
        }
    }
}

async fn read_loop<R: AsyncRead + Unpin>(
    reader: R,
    pending: Arc<Pending>,
    out: mpsc::Sender<Vec<u8>>,
    alive: Arc<AtomicBool>,
    stale: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        // `take` bounds one message; a longer one has no newline within the limit.
        let read = (&mut reader)
            .take(MAX_MESSAGE_BYTES)
            .read_until(b'\n', &mut line)
            .await;
        match read {
            Ok(0) | Err(_) => break,
            Ok(_) if line.last() != Some(&b'\n') && line.len() as u64 >= MAX_MESSAGE_BYTES => break,
            Ok(_) => {}
        }
        let Ok(message) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        route(message, &pending, &out, &stale).await;
    }
    alive.store(false, Ordering::Relaxed);
    // Dropping every sender wakes each waiting caller with `Closed`.
    pending.lock().clear();
}

async fn route(message: Value, pending: &Pending, out: &mpsc::Sender<Vec<u8>>, stale: &AtomicBool) {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").filter(|id| !id.is_null());
    match (method, id) {
        (Some(_), Some(id)) => {
            // A server asking us something (roots, sampling): we advertise nothing, so
            // say so rather than leave it waiting.
            let refusal = json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "this client offers no capabilities" },
            });
            let _ = out.send(encode(&refusal)).await;
        }
        (Some("notifications/progress"), None) => {
            let token = message
                .pointer("/params/progressToken")
                .and_then(Value::as_u64);
            let Some(token) = token else { return };
            if let Some(slot) = pending.lock().get(&token) {
                slot.activity.notify_one();
            }
        }
        (Some("notifications/tools/list_changed"), None) => stale.store(true, Ordering::Relaxed),
        (None, Some(id)) => {
            let Some(slot) = id.as_u64().and_then(|id| pending.lock().remove(&id)) else {
                return;
            };
            let _ = slot.reply.send(reply_of(message));
        }
        _ => {}
    }
}

fn reply_of(mut message: Value) -> Reply {
    if let Some(error) = message.get("error") {
        return Err(CallError::Rpc {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no detail")
                .to_string(),
        });
    }
    Ok(message
        .get_mut("result")
        .map(Value::take)
        .unwrap_or(Value::Null))
}

#[cfg(test)]
#[path = "conn_tests.rs"]
mod conn_tests;
