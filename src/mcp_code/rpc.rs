//! JSON-RPC plumbing for the server side: framing, replies, and progress ticks.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Value,
}

pub fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A shared, serialised stdout: replies and progress ticks come from different tasks.
pub type Out<W> = Arc<Mutex<W>>;

/// One line per message, flushed: the client reads a pipe, not a stream it can poll.
pub async fn write<W: AsyncWrite + Unpin>(out: &Out<W>, message: &Value) {
    let Ok(mut bytes) = serde_json::to_vec(message) else {
        return;
    };
    bytes.push(b'\n');
    let mut out = out.lock().await;
    let _ = out.write_all(&bytes).await;
    let _ = out.flush().await;
}

/// How often to tick. Inside the shortest measured client timeout (OpenCode: 60s).
const TICK: Duration = Duration::from_secs(10);

/// Emit `notifications/progress` for as long as this future is polled, so a client that
/// resets its clock on progress does not give up on a long script. Race it against the
/// work; the `Infallible` return says it never finishes by itself.
pub async fn tick_until_dropped<W: AsyncWrite + Unpin>(
    out: &Out<W>,
    token: Option<Value>,
) -> Infallible {
    let Some(token) = token else {
        return std::future::pending().await;
    };
    let mut step = 0_u64;
    loop {
        tokio::time::sleep(TICK).await;
        step += 1;
        let note = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": { "progressToken": token, "progress": step, "message": "script still running" },
        });
        write(out, &note).await;
    }
}
