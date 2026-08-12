//! Idle embedded-Neovim reaper.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, info};

use super::pool::NvimUiPool;

const REAP_INTERVAL: Duration = Duration::from_secs(60);
const DEFAULT_IDLE_SECS: u64 = 600;

fn enabled() -> bool {
    std::env::var("OPMAN_NVIM_REAP")
        .map(|value| value != "0")
        .unwrap_or(true)
}

pub fn idle_ttl() -> Duration {
    let secs = std::env::var("OPMAN_NVIM_IDLE_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_IDLE_SECS);
    Duration::from_secs(secs)
}

pub fn spawn(pool: Arc<NvimUiPool>) {
    if !enabled() {
        debug!("nvim ui reaper disabled by OPMAN_NVIM_REAP=0");
        return;
    }
    let ttl = idle_ttl();
    tokio::spawn(async move {
        let mut ticker = interval(REAP_INTERVAL);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let reaped = pool.sweep(ttl).await;
            if reaped > 0 {
                info!(reaped, "nvim ui: shut down idle sessions");
            }
        }
    });
}
