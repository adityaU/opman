//! What an agent does to a page: read it, and act on what it read.
//!
//! Split from `tab.rs`, which keeps the target's lifecycle and its navigation. The line
//! is the one the MCP tools already draw — `browser_snapshot` and `browser_click` are the
//! same conversation with a page, while opening and closing a tab is not.
//!
//! Nothing here holds state: a `[ref=eN]` is resolved against the live DOM every time it
//! is used, which is what makes a stale ref an error rather than a click on whatever has
//! since moved into that spot.
//!
//! Declared inside `tab.rs` rather than beside it: these methods reach into the target's
//! own session, and a sibling module would need those fields made visible to the whole
//! crate to do it.

use serde_json::json;

use super::{Tab, SETTLE};
use crate::browser::input;
use crate::browser::types::{PageSnapshot, PageText, SnapshotOptions};

const OUTLINE_JS: &str = include_str!("outline.js");
const RESOLVE_JS: &str = include_str!("resolve.js");
const READABLE_JS: &str = include_str!("readable.js");
const DEFAULT_TEXT_CHARS: usize = 8_000;

impl Tab {
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
