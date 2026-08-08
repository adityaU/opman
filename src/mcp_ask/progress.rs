//! Keeping a tool call alive while a human decides.
//!
//! A question is the longest a tool call ever legitimately runs, and the measured client
//! ceilings are far shorter than a person: OpenCode cancels a silent call after 60
//! seconds. MCP lets a client treat `notifications/progress` as a reason to reset that
//! clock, so the wait ticks. Progress is only legal for a `progressToken` the client
//! supplied, so a client that sends none simply gets no ticks — and the registry's
//! per-server timeout is what covers it there.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

/// How often to tick. Comfortably inside the shortest measured base timeout.
const TICK: Duration = Duration::from_secs(10);

/// Emit progress for as long as this future is polled. The [`Infallible`] return says it
/// never completes: race it against the work with `tokio::select!` and the ticker is
/// dropped the moment the answer lands.
pub(super) async fn tick_until_dropped<W>(
    stdout: &Arc<Mutex<W>>,
    token: Option<Value>,
) -> Infallible
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let Some(token) = token else {
        return std::future::pending().await;
    };
    let mut step = 0_u64;
    loop {
        tokio::time::sleep(TICK).await;
        step += 1;
        super::write_rpc(stdout, &notification(&token, step)).await;
    }
}

/// `progress` must strictly increase; `total` is omitted because the wait is open-ended —
/// the documented shape for work of unknown length.
fn notification(token: &Value, step: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {
            "progressToken": token,
            "progress": step,
            "message": "waiting for the user to answer in opman",
        }
    })
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod progress_tests;
