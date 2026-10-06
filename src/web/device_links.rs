//! Lends this machine's device browser to every configured remote server.
//!
//! Only in device mode (the desktop app). For each remote in the [`Registry`] one task
//! keeps an outbound WebSocket to `{remote}/api/browser/device` alive and pipes it to the
//! local browser ([`crate::browser::run_device_link_loop`]). The link is outbound, so it
//! works when the remote cannot reach this machine. Adding, editing or removing a server
//! starts, restarts or stops its link.

use std::collections::HashMap;

use tokio::sync::oneshot;

use crate::browser::{run_device_link_loop, BrowserMode, BrowserPool};
use crate::remote::{Registry, ServerId, ServerSnapshot};

const DEVICE_PATH: &str = "/api/browser/device";

/// One running link. Dropping it drops `stop`, which ends the link's loop.
struct Link {
    snapshot: ServerSnapshot,
    _stop: oneshot::Sender<()>,
}

/// Start the supervisor. Does nothing outside device mode.
pub fn spawn(remote: Registry, browser: BrowserPool) {
    if browser.mode() != BrowserMode::Device {
        return;
    }
    tokio::spawn(supervise(remote, browser));
}

async fn supervise(remote: Registry, browser: BrowserPool) {
    let mut changes = remote.subscribe();
    let mut links: HashMap<ServerId, Link> = HashMap::new();
    loop {
        let wanted = remote.servers().await;
        // Removed or edited servers: their link stops when the entry is dropped.
        links.retain(|_, link| wanted.contains(&link.snapshot));
        for snapshot in wanted {
            if links.contains_key(&snapshot.id) {
                continue;
            }
            let stop = start(&remote, &browser, snapshot.id.clone());
            links.insert(snapshot.id.clone(), Link { snapshot, _stop: stop });
        }
        if changes.changed().await.is_err() {
            return;
        }
    }
}

fn start(remote: &Registry, browser: &BrowserPool, id: ServerId) -> oneshot::Sender<()> {
    let (stop, stopped) = oneshot::channel::<()>();
    let remote = remote.clone();
    let browser = browser.clone();
    tokio::spawn(async move {
        let connect = || {
            let (remote, id) = (remote.clone(), id.clone());
            async move { Ok(remote.connect_ws(&id, DEVICE_PATH).await?) }
        };
        // A dropped sender resolves the receiver too: that is the shutdown signal.
        let shutdown = async {
            let _ = stopped.await;
        };
        if let Err(error) = run_device_link_loop(browser, connect, shutdown).await {
            tracing::warn!(server = %id, %error, "device link to remote server stopped");
        }
    });
    stop
}
