//! One pane's page, and the little the app needs to know about it.
//!
//! Split from `pool.rs`, which owns the registry of these and the browser behind them.
//! The state here is deliberately thin: the tab is the truth about where the page is, and
//! this is the cache the header renders from between navigations — kept because asking
//! Chromium for a title on every repaint would put a round trip in the render path.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::tab::Tab;
use super::types::{PaneInfo, RenderMode};

/// A pane's tab plus the bits of state the UI shows in the header.
pub struct Pane {
    tab: Tab,
    project: Arc<str>,
    meta: Mutex<Meta>,
}

struct Meta {
    url: String,
    title: String,
    mode: RenderMode,
}

impl Pane {
    /// A pane on a fresh tab. Screencast until a navigation's framing probe says otherwise
    /// — the mode that always renders something is the right thing to start on.
    pub(super) fn new(tab: Tab, project: &str) -> Self {
        Self {
            tab,
            project: Arc::from(project),
            meta: Mutex::new(Meta {
                url: super::pool::BLANK.into(),
                title: String::new(),
                mode: RenderMode::Screencast,
            }),
        }
    }

    pub fn tab(&self) -> &Tab {
        &self.tab
    }

    /// The project this browser belongs to. Browsers are per project, so this is what
    /// lets an agent working in one repo reach that repo's browser and no other.
    pub fn project(&self) -> &Arc<str> {
        &self.project
    }

    /// Where the tab actually is, as opposed to where a reopened widget remembers it.
    pub async fn current_url(&self) -> String {
        self.meta.lock().await.url.clone()
    }

    pub async fn mode(&self) -> RenderMode {
        self.meta.lock().await.mode
    }

    /// Force screencast on a site the probe thought was framable — the escape hatch for
    /// pages that break inside an iframe for reasons no header advertises.
    pub async fn set_mode(&self, mode: RenderMode) {
        self.meta.lock().await.mode = mode;
    }

    pub(super) async fn record(&self, url: String, title: String, mode: RenderMode) {
        let mut meta = self.meta.lock().await;
        meta.url = url;
        meta.title = title;
        meta.mode = mode;
    }

    pub(super) async fn info(&self, pane_id: Arc<str>) -> PaneInfo {
        let meta = self.meta.lock().await;
        PaneInfo {
            pane_id,
            project: Arc::clone(&self.project),
            url: meta.url.clone(),
            title: meta.title.clone(),
            mode: meta.mode,
        }
    }
}
