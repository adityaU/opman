//! Attaching and detaching device links — split from `pool.rs` for length. The engine
//! decides which browser is current; this half keeps the pane map in step with it.

use std::sync::Arc;

use super::super::engine::{Detach, DeviceLink, LinkId};
use super::super::pane::Pane;
use super::BrowserPool;

impl BrowserPool {
    /// Attach a device link: from now on panes open over it. Panes on the previous
    /// browser are closed — the UI reopens them, on the device.
    pub async fn attach_link(&self) -> DeviceLink {
        let link = self.engine.lock().await.attach_link();
        let stale: Vec<Arc<Pane>> = self
            .panes
            .write()
            .await
            .drain()
            .map(|(_, pane)| pane)
            .collect();
        if !stale.is_empty() {
            // Closing over a link that is going away can wait out a call timeout; nobody
            // should wait on that.
            tokio::spawn(async move {
                for pane in stale {
                    pane.tab().close().await;
                }
            });
        }
        tracing::info!(link = ?link.id, "device browser link attached");
        link
    }

    /// The link's transport ended. Its panes went with it — nothing to close them on.
    pub async fn detach_link(&self, id: LinkId) {
        let mut engine = self.engine.lock().await;
        if engine.detach_link(id) == Detach::Detached {
            self.panes.write().await.clear();
            tracing::info!(link = ?id, "device browser link detached");
        }
    }
}
