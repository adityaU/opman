//! Idle language-server reaper.
//!
//! A warm rust-analyzer answers instantly and costs a gigabyte or more of RSS.
//! Keeping it alive while someone is reading code is worth every byte; keeping
//! it alive overnight because a file was opened once is not. This sweeps the
//! pool on the same cadence and with the same env conventions as the Claude
//! background-agent reaper in [`crate::claude_engine::reaper`].

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, info};

use super::pool::LspPool;

/// How often the reaper sweeps.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

/// Default idle TTL before a server is shut down.
const DEFAULT_IDLE_SECS: u64 = 600;

/// `OPMAN_LSP_REAP=0` disables the reaper entirely.
fn enabled() -> bool {
    std::env::var("OPMAN_LSP_REAP")
        .map(|value| value != "0")
        .unwrap_or(true)
}

/// Idle TTL, overridable via `OPMAN_LSP_IDLE_SECS`.
pub fn idle_ttl() -> Duration {
    let secs = std::env::var("OPMAN_LSP_IDLE_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_IDLE_SECS);
    Duration::from_secs(secs)
}

/// Start the sweep loop. Returns immediately; the work happens on its own task.
pub fn spawn(pool: Arc<LspPool>) {
    if !enabled() {
        debug!("lsp reaper disabled by OPMAN_LSP_REAP=0");
        return;
    }
    let ttl = idle_ttl();
    tokio::spawn(async move {
        let mut ticker = interval(REAP_INTERVAL);
        // The first tick fires immediately; skip it so a server started
        // moments ago is never swept before it has been used.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let reaped = pool.sweep(ttl).await;
            if reaped > 0 {
                info!(reaped, "lsp: shut down idle language servers");
            }
        }
    });
}
