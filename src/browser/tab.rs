//! One page, one pane. Owns a CDP target and turns it into the handful of operations a
//! pane header and an MCP tool actually need.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::cdp::Cdp;
use super::input;
use super::screencast::Screencast;
use super::types::{PageSnapshot, PageText, SnapshotOptions};

const OUTLINE_JS: &str = include_str!("outline.js");
const RESOLVE_JS: &str = include_str!("resolve.js");
const READABLE_JS: &str = include_str!("readable.js");

/// Longest a navigation may take before the caller gets the page as-is. A slow page is
/// still worth snapshotting; a hung one must not hold the tool call open.
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
/// Let script-driven pages paint after `load` before reading them.
const SETTLE: Duration = Duration::from_millis(350);
const DEFAULT_TEXT_CHARS: usize = 8_000;

pub struct Tab {
    cdp: Cdp,
    session_id: Arc<str>,
    target_id: String,
    screencast: Screencast,
}

impl Tab {
    /// Create a page and attach to it in flat mode.
    pub async fn open(cdp: Cdp, width: u32, height: u32) -> anyhow::Result<Self> {
        // No width/height here: `Target.createTarget` only accepts a size for a new
        // *window*, and the viewport is set by the emulation override below anyway —
        // which is also what a later `resize` adjusts.
        let created = cdp
            .call("Target.createTarget", json!({ "url": "about:blank" }))
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
        tab.call(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": false }),
        )
        .await?;
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
        self.eval_json::<String>("JSON.stringify(document.title)").await
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

    /// The compact outline. This is what an LLM reads instead of HTML.
    pub async fn snapshot(&self, options: SnapshotOptions) -> anyhow::Result<PageSnapshot> {
        let opts = json!({
            "maxNodes": options.max_nodes,
            "maxChars": options.max_chars,
            "maxTextLen": options.max_text_len,
            "viewportOnly": options.viewport_only,
        });
        let script = OUTLINE_JS.replace("OPTIONS", &opts.to_string());
        let raw: RawSnapshot = self.eval_json(&script).await?;
        Ok(raw.into())
    }

    /// Main-content prose, for reading.
    pub async fn read_text(&self, max_chars: Option<usize>) -> anyhow::Result<PageText> {
        let limit = max_chars.unwrap_or(DEFAULT_TEXT_CHARS);
        let script = READABLE_JS.replace("MAX_CHARS", &limit.to_string());
        self.eval_json(&script).await
    }

    /// Resolve a `[ref=eN]` handle to viewport coordinates, scrolling it into view.
    pub(super) async fn resolve(&self, reference: &str) -> anyhow::Result<input::Target> {
        let script = RESOLVE_JS.replace("REF", &json!(reference).to_string());
        let resolved: input::Resolved = self.eval_json(&script).await?;
        resolved.into_target()
    }

    pub async fn click_ref(&self, reference: &str) -> anyhow::Result<()> {
        let target = self.resolve(reference).await?;
        input::click(&self.cdp, &self.session_id, target.x, target.y).await
    }

    /// Focus a field, clear it, and type. Uses `Input.insertText` for the body of the
    /// value and real key events for Enter, so both React-style listeners and plain form
    /// submits see what they expect.
    pub async fn type_ref(&self, reference: &str, text: &str, submit: bool) -> anyhow::Result<()> {
        let target = self.resolve(reference).await?;
        if !target.editable {
            return Err(anyhow::anyhow!(
                "ref {reference} is a {} — it cannot be typed into",
                target.tag
            ));
        }
        input::click(&self.cdp, &self.session_id, target.x, target.y).await?;
        input::select_all(&self.cdp, &self.session_id).await?;
        input::insert_text(&self.cdp, &self.session_id, text).await?;
        if submit {
            input::press(&self.cdp, &self.session_id, "Enter").await?;
            tokio::time::sleep(SETTLE).await;
        }
        Ok(())
    }

    pub async fn press_key(&self, key: &str) -> anyhow::Result<()> {
        input::press(&self.cdp, &self.session_id, key).await
    }

    pub async fn scroll(&self, x: i64, y: i64, delta_y: i64) -> anyhow::Result<()> {
        input::scroll(&self.cdp, &self.session_id, x, y, delta_y).await
    }

    pub async fn mouse(&self, kind: input::MouseKind, x: i64, y: i64) -> anyhow::Result<()> {
        input::mouse(&self.cdp, &self.session_id, kind, x, y).await
    }

    pub async fn insert_text(&self, text: &str) -> anyhow::Result<()> {
        input::insert_text(&self.cdp, &self.session_id, text).await
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

    pub async fn resize(&self, width: u32, height: u32) -> anyhow::Result<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": false }),
        )
        .await
        .map(drop)
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

/// The JS side speaks camelCase; keep the boundary explicit rather than decorating the
/// public type with rename attributes it does not otherwise need.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSnapshot {
    url: String,
    title: String,
    scroll_y: i64,
    scroll_height: i64,
    viewport_height: i64,
    ref_count: usize,
    truncated: bool,
    outline: String,
}

impl From<RawSnapshot> for PageSnapshot {
    fn from(raw: RawSnapshot) -> Self {
        Self {
            url: raw.url,
            title: raw.title,
            scroll_y: raw.scroll_y,
            scroll_height: raw.scroll_height,
            viewport_height: raw.viewport_height,
            ref_count: raw.ref_count,
            truncated: raw.truncated,
            outline: raw.outline,
        }
    }
}
