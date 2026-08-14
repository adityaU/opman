//! Live frames for panes that cannot use an iframe.
//!
//! Streaming is reference-counted against attached viewers: the first `viewer()` starts
//! the CDP screencast, the last one dropped stops it. A backgrounded pane therefore costs
//! nothing, which is the point — encoding JPEG for a hidden tab is pure waste, and a
//! workspace can hold many panes.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{Mutex, Notify};

use super::cdp::Cdp;

/// Frames are JPEG at this quality — legible text at a fraction of PNG's bytes.
const QUALITY: u32 = 60;
/// Cap the encoded width; the pane scales to fit anyway.
const MAX_WIDTH: u32 = 1280;

#[derive(Default)]
struct State {
    viewers: AtomicUsize,
    /// Base64 JPEG. `Arc<str>` so handing a frame to a stream never copies it.
    frame: Mutex<Option<Arc<str>>>,
    /// Bumped per frame; a stream compares against its own last seen value instead of
    /// re-sending an unchanged image every tick.
    version: AtomicU64,
    ready: Notify,
}

#[derive(Clone)]
pub struct Screencast {
    cdp: Cdp,
    session_id: Arc<str>,
    state: Arc<State>,
}

/// Keeps the screencast running for as long as it is held.
pub struct Viewer {
    screencast: Screencast,
}

impl Screencast {
    pub(super) fn new(cdp: Cdp, session_id: Arc<str>) -> Self {
        Self {
            cdp,
            session_id,
            state: Arc::new(State::default()),
        }
    }

    /// Attach. Starts the stream if this is the first viewer.
    pub async fn viewer(&self) -> Viewer {
        if self.state.viewers.fetch_add(1, Ordering::AcqRel) == 0 {
            self.start().await;
        }
        Viewer {
            screencast: self.clone(),
        }
    }

    /// The newest frame and its version, or `None` before the first one arrives.
    pub async fn latest(&self) -> Option<(Arc<str>, u64)> {
        let frame = self.state.frame.lock().await.clone()?;
        Some((frame, self.state.version.load(Ordering::Acquire)))
    }

    /// Wait until a frame newer than `seen` exists. Returns it, or `None` if the wait was
    /// cut short — the caller loops, so a spurious wake is harmless.
    pub async fn next_after(&self, seen: u64) -> Option<(Arc<str>, u64)> {
        if self.state.version.load(Ordering::Acquire) > seen {
            return self.latest().await;
        }
        self.state.ready.notified().await;
        self.latest().await.filter(|(_, version)| *version > seen)
    }

    async fn start(&self) {
        let started = self
            .cdp
            .call_on(
                &self.session_id,
                "Page.startScreencast",
                json!({ "format": "jpeg", "quality": QUALITY, "maxWidth": MAX_WIDTH, "everyNthFrame": 1 }),
            )
            .await;
        if started.is_err() {
            return;
        }

        let mut events = self.cdp.subscribe();
        let session_id = Arc::clone(&self.session_id);
        let state = Arc::clone(&self.state);
        let cdp = self.cdp.clone();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if state.viewers.load(Ordering::Acquire) == 0 {
                    break;
                }
                if &*event.method != "Page.screencastFrame" {
                    continue;
                }
                if event.session_id.as_deref() != Some(session_id.as_ref()) {
                    continue;
                }
                // Ack first: Chromium withholds the next frame until the previous one is
                // acknowledged, so acking after the store would halve the frame rate.
                if let Some(ack) = event.params.get("sessionId") {
                    cdp.notify_on(
                        &session_id,
                        "Page.screencastFrameAck",
                        json!({ "sessionId": ack }),
                    );
                }
                let Some(data) = event.params.get("data").and_then(Value::as_str) else {
                    continue;
                };
                *state.frame.lock().await = Some(Arc::from(data));
                state.version.fetch_add(1, Ordering::AcqRel);
                state.ready.notify_waiters();
            }
        });
    }

    /// Stop streaming unconditionally. Called on tab close; viewers are gone by then.
    pub async fn stop(&self) {
        let _ = self
            .cdp
            .call_on(&self.session_id, "Page.stopScreencast", json!({}))
            .await;
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        if self.screencast.state.viewers.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        // Last viewer left: stop the encoder. Detached because Drop cannot await, and the
        // stop is advisory — a leaked screencast would only waste frames, not correctness.
        let screencast = self.screencast.clone();
        tokio::spawn(async move { screencast.stop().await });
    }
}
