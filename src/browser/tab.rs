//! One page, one pane. Owns a CDP target and turns it into the handful of operations a
//! pane header and an MCP tool actually need.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::cdp::Cdp;
use super::screencast::Screencast;
use super::types::Viewport;

/// Longest a navigation may take before the caller gets the page as-is. A slow page is
/// still worth snapshotting; a hung one must not hold the tool call open.
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
/// Let script-driven pages paint after `load` before reading them.
const SETTLE: Duration = Duration::from_millis(350);

pub struct Tab {
    cdp: Cdp,
    session_id: Arc<str>,
    target_id: String,
    screencast: Screencast,
}

impl Tab {
    /// Create a page and attach to it in flat mode.
    pub async fn open(cdp: Cdp, viewport: Viewport) -> anyhow::Result<Self> {
        // Every pane gets its own window, not a background tab in a shared one: a headed
        // browser paints only the foreground tab, so panes sharing a window would leave
        // all but one screencast frozen. Nothing is visible either way — the windows live
        // on a virtual display — and `Target.createTarget` accepts a size only for a new
        // window, so this is also the one place the initial extent can be set.
        let created = cdp
            .call(
                "Target.createTarget",
                json!({
                    "url": "about:blank",
                    "newWindow": true,
                    "width": viewport.width(),
                    "height": viewport.height(),
                }),
            )
            .await?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Target.createTarget returned no targetId"))?
            .to_owned();

        let attached = cdp
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
            )
            .await?;
        let session_id: Arc<str> = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Target.attachToTarget returned no sessionId"))?
            .into();

        let tab = Self {
            screencast: Screencast::new(cdp.clone(), Arc::clone(&session_id)),
            cdp,
            session_id,
            target_id,
        };
        tab.call("Page.enable", json!({})).await?;
        tab.call("Runtime.enable", json!({})).await?;
        tab.resize(viewport).await?;
        Ok(tab)
    }

    pub fn session_id(&self) -> &Arc<str> {
        &self.session_id
    }

    pub fn screencast(&self) -> &Screencast {
        &self.screencast
    }

    async fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.cdp.call_on(&self.session_id, method, params).await
    }

    /// Evaluate an expression that returns a JSON string, and parse it.
    async fn eval_json<T: serde::de::DeserializeOwned>(&self, script: &str) -> anyhow::Result<T> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true,
                }),
            )
            .await?;

        if let Some(details) = result.get("exceptionDetails") {
            let text = details
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(Value::as_str)
                .unwrap_or("page script threw");
            return Err(anyhow::anyhow!("{text}"));
        }
        let raw = result
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("page script returned no value"))?;
        serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("malformed page result: {e}"))
    }

    /// Navigate and wait for `load`, then let the page settle.
    pub async fn navigate(&self, url: &str) -> anyhow::Result<()> {
        let mut events = self.cdp.subscribe();
        let result = self.call("Page.navigate", json!({ "url": url })).await?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            return Err(anyhow::anyhow!("navigation to {url} failed: {error}"));
        }

        let session = Arc::clone(&self.session_id);
        let wait = async {
            while let Ok(event) = events.recv().await {
                let ours = event.session_id.as_deref() == Some(session.as_ref());
                if ours && &*event.method == "Page.loadEventFired" {
                    return;
                }
            }
        };
        // A timed-out load is not an error: many pages keep a socket open forever and are
        // perfectly readable. The caller sees whatever has rendered.
        let _ = tokio::time::timeout(LOAD_TIMEOUT, wait).await;
        tokio::time::sleep(SETTLE).await;
        Ok(())
    }

    /// The page's title, without paying for a full outline. Used when adopting a running
    /// tab, where the caller wants the header text and nothing else.
    pub async fn title(&self) -> anyhow::Result<String> {
        self.eval_json::<String>("JSON.stringify(document.title)")
            .await
    }

    pub async fn go_back(&self) -> anyhow::Result<()> {
        self.history_step(-1).await
    }

    pub async fn go_forward(&self) -> anyhow::Result<()> {
        self.history_step(1).await
    }

    async fn history_step(&self, delta: i64) -> anyhow::Result<()> {
        let history = self.call("Page.getNavigationHistory", json!({})).await?;
        let current = history
            .get("currentIndex")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let entries = history
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default() as i64;

        let target = current + delta;
        if target < 0 || target >= entries {
            return Err(anyhow::anyhow!("no page in that direction"));
        }
        let id = history
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|list| list.get(target as usize))
            .and_then(|entry| entry.get("id"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("history entry {target} has no id"))?;

        self.call("Page.navigateToHistoryEntry", json!({ "entryId": id }))
            .await?;
        tokio::time::sleep(SETTLE).await;
        Ok(())
    }

    pub async fn reload(&self) -> anyhow::Result<()> {
        self.call("Page.reload", json!({ "ignoreCache": false }))
            .await?;
        tokio::time::sleep(SETTLE).await;
        Ok(())
    }

    /// A downscaled JPEG, base64. Only taken when explicitly asked for — a screenshot
    /// costs roughly as many tokens as a hundred outlines.
    pub async fn screenshot(&self, quality: u8) -> anyhow::Result<String> {
        let result = self
            .call(
                "Page.captureScreenshot",
                json!({ "format": "jpeg", "quality": quality.min(100) }),
            )
            .await?;
        result
            .get("data")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Page.captureScreenshot returned no data"))
    }

    /// Match the page's viewport to the pane, at the pane's own pixel density.
    ///
    /// Two separate effects, easy to confuse: the override decides the page's *layout* and
    /// the `devicePixelRatio` it reports (so it picks retina assets), while the capture
    /// width decides how many pixels a screencast frame actually carries.
    pub async fn resize(&self, viewport: Viewport) -> anyhow::Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": viewport.width(),
                "height": viewport.height(),
                "deviceScaleFactor": viewport.scale(),
                "mobile": false,
            }),
        )
        .await?;
        self.screencast
            .set_capture_width(viewport.capture_width())
            .await;
        Ok(())
    }

    /// Close the page. The pool drops the entry immediately afterwards, so nothing can
    /// reach the now-invalid session.
    pub async fn close(&self) {
        self.screencast.stop().await;
        let _ = self
            .cdp
            .call("Target.closeTarget", json!({ "targetId": self.target_id }))
            .await;
    }
}

#[path = "tab_page.rs"]
mod tab_page;
