//! Pane → tab registry, and the one lazily-launched browser behind all of them.
//!
//! Nothing starts until a pane is actually opened: a workspace with no browser widget
//! never pays for Chromium. The pool is the only place that knows a pane id, so panes
//! stay addressable by both the UI and the MCP tools — that shared addressing is what
//! makes "the agent drives the pane you are watching" work.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use super::engine::{BrowserStatus, Engine};
use super::mode::BrowserMode;
use super::pane::Pane;
use super::tab::Tab;
use super::types::{PaneInfo, RenderMode, Viewport};

/// A tab that has never been anywhere. Distinguishing it is what lets `open` tell a fresh
/// pane (send it to the saved URL) from a live one (adopt whatever it is showing).
pub const BLANK: &str = "about:blank";

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 800;
/// Framability probes must not hold up opening a pane; a slow site just gets an iframe
/// attempt, which the pane can still flip to screencast by hand.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

/// Whether [`BrowserPool::open`] created the tab or handed back a running one.
///
/// The caller needs to know: connecting a reopened pane must adopt whatever page the tab
/// is already on — possibly one an agent navigated to — rather than resetting it to
/// wherever the widget was last saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    Created,
    Adopted,
}

/// Shared, cloneable handle. Put one on `ServerState`.
#[derive(Clone)]
pub struct BrowserPool {
    engine: Arc<Mutex<Engine>>,
    panes: Arc<RwLock<HashMap<Arc<str>, Arc<Pane>>>>,
    http: reqwest::Client,
    #[cfg_attr(not(test), allow(dead_code))]
    mode: BrowserMode,
}

impl BrowserPool {
    /// A pool in server mode — today's private Chromium on a virtual display.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_mode(http, BrowserMode::Server)
    }

    pub fn with_mode(http: reqwest::Client, mode: BrowserMode) -> Self {
        Self::build(http, mode, None)
    }

    fn build(
        http: reqwest::Client,
        mode: BrowserMode,
        profile: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            engine: Arc::new(Mutex::new(Engine::new(mode, profile))),
            panes: Arc::new(RwLock::new(HashMap::new())),
            http,
            mode,
        }
    }

    /// A pool with its own profile directory. Chromium takes an exclusive lock on a
    /// profile, so two pools sharing one would leave the second unable to start.
    #[cfg(test)]
    pub fn with_profile(http: reqwest::Client, profile: std::path::PathBuf) -> Self {
        Self::build(http, BrowserMode::Server, Some(profile))
    }

    #[cfg(test)]
    pub fn with_mode_and_profile(
        http: reqwest::Client,
        mode: BrowserMode,
        profile: std::path::PathBuf,
    ) -> Self {
        Self::build(http, mode, Some(profile))
    }

    // Read by the device-link relay, which the remote-server supervisor drives.
    pub fn mode(&self) -> BrowserMode {
        self.mode
    }

    /// Which browser the panes are on, for the UI.
    pub async fn status(&self) -> BrowserStatus {
        self.engine.lock().await.status()
    }

    /// A client to open a pane on. A browser that died since the last call takes every tab
    /// with it, so the pane map is dropped whenever the epoch moved.
    async fn lease(&self) -> anyhow::Result<super::engine::Lease> {
        let mut engine = self.engine.lock().await;
        let before = engine.epoch();
        let lease = engine.lease().await;
        if engine.epoch() != before {
            self.panes.write().await.clear();
        }
        lease
    }

    /// The local browser's browser-level DevTools endpoint, launching it if needed. What
    /// [`super::relay`] dials to serve a remote's device link.
    pub async fn local_endpoint(&self) -> anyhow::Result<String> {
        self.engine.lock().await.local_endpoint().await
    }

    /// The pane's tab, creating it on first use.
    ///
    /// Reports which happened, because a reopened widget must adopt a running tab rather
    /// than steer it back to where the widget was saved.
    pub async fn open(&self, pane_id: &str, project: &str) -> anyhow::Result<(Arc<Pane>, Opened)> {
        if let Some(pane) = self.panes.read().await.get(pane_id) {
            return Ok((Arc::clone(pane), Opened::Adopted));
        }

        let lease = self.lease().await?;
        let viewport = Viewport::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, None);
        let tab = Tab::open(lease.cdp, viewport, lease.placement).await?;
        let pane = Arc::new(Pane::new(tab, project));

        let engine = self.engine.lock().await;
        // The browser changed under the open (a link attached or dropped): this tab
        // belongs to a browser the panes have already left.
        if engine.epoch() != lease.epoch {
            drop(engine);
            pane.tab().close().await;
            anyhow::bail!("the browser changed while the pane was opening; open it again");
        }
        let mut panes = self.panes.write().await;
        drop(engine);
        // Two panes racing on the same id: keep whoever landed first, close the loser's
        // tab rather than leaking it.
        if let Some(existing) = panes.get(pane_id) {
            let existing = Arc::clone(existing);
            drop(panes);
            pane.tab().close().await;
            return Ok((existing, Opened::Adopted));
        }
        panes.insert(Arc::from(pane_id), Arc::clone(&pane));
        Ok((pane, Opened::Created))
    }

    /// The open browser belonging to a project, if there is one.
    ///
    /// Browsers are per project rather than per pane, so this is how an agent reaches
    /// "the browser for the repo I am working in" without being told an opaque id.
    pub async fn for_project(&self, project: &str) -> Option<(Arc<str>, Arc<Pane>)> {
        self.panes
            .read()
            .await
            .iter()
            .find(|(_, pane)| pane.project().as_ref() == project)
            .map(|(id, pane)| (Arc::clone(id), Arc::clone(pane)))
    }

    /// An already-open pane, without creating one. Tools use this so an agent cannot
    /// silently spawn tabs by naming a pane that does not exist.
    pub async fn get(&self, pane_id: &str) -> Option<Arc<Pane>> {
        self.panes.read().await.get(pane_id).cloned()
    }

    /// Navigate a pane and record where it ended up, including whether the destination
    /// can be shown in an iframe.
    pub async fn navigate(
        &self,
        pane_id: &str,
        project: &str,
        url: &str,
    ) -> anyhow::Result<RenderMode> {
        let url = normalize_url(url)?;
        let (pane, _) = self.open(pane_id, project).await?;

        // Probe and navigate together: the probe is a plain HTTP round trip and has no
        // reason to serialise behind the page load.
        let (mode, navigated) = tokio::join!(self.probe(&url), pane.tab().navigate(&url));
        navigated?;

        let snapshot = pane.tab().snapshot(Default::default()).await;
        let (final_url, title) = match snapshot {
            Ok(page) => (page.url, page.title),
            Err(_) => (url, String::new()),
        };
        pane.record(final_url, title, mode).await;
        Ok(mode)
    }

    /// Ask the site whether it allows framing. Failures resolve to screencast — the mode
    /// that always works — so an unreachable probe never leaves a pane blank.
    async fn probe(&self, url: &str) -> RenderMode {
        let request = self.http.get(url).timeout(PROBE_TIMEOUT).send();
        let Ok(response) = request.await else {
            return RenderMode::Screencast;
        };
        let headers = response.headers();
        let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
        RenderMode::from_headers(header("x-frame-options"), header("content-security-policy"))
    }

    /// Every open pane, for `browser_list_panes`.
    pub async fn list(&self) -> Vec<PaneInfo> {
        let panes = self.panes.read().await;
        let mut out = Vec::with_capacity(panes.len());
        for (id, pane) in panes.iter() {
            out.push(pane.info(Arc::clone(id)).await);
        }
        out.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
        out
    }

    /// Close one pane's tab. Idempotent: closing an already-closed pane is a no-op, which
    /// matters because the UI closes on unmount and the user can also close explicitly.
    pub async fn close(&self, pane_id: &str) {
        let Some(pane) = self.panes.write().await.remove(pane_id) else {
            return;
        };
        pane.tab().close().await;
    }

    /// Tear down every tab and the browser itself. Called on server shutdown.
    pub async fn shutdown(&self) {
        for (_, pane) in self.panes.write().await.drain() {
            pane.tab().close().await;
        }
        self.engine.lock().await.shutdown().await;
    }
}

/// The pane id for a project's browser.
///
/// Derived rather than random, and derived the same way on both sides — the widget
/// computes it from its `projectPath`, the MCP server from the directory it was launched
/// in. That shared derivation is the whole mechanism behind "one browser per project":
/// nothing has to be told an id for the agent and the pane to land on the same tab.
pub fn pane_id_for_project(project: &str) -> String {
    format!("proj:{project}")
}

/// Accept what a person would type. A bare host is a URL with the scheme left off, not a
/// search — guessing otherwise would send the user's input to a search engine.
pub fn normalize_url(input: &str) -> anyhow::Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("empty URL"));
    }
    let candidate = match trimmed.split_once("://") {
        Some((scheme, _)) if scheme.eq_ignore_ascii_case("about") => trimmed.to_owned(),
        Some(_) => trimmed.to_owned(),
        None if trimmed.starts_with("about:") => trimmed.to_owned(),
        None => format!("https://{trimmed}"),
    };
    match url::Url::parse(&candidate) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https" | "about" | "file") => {
            Ok(candidate)
        }
        Ok(parsed) => Err(anyhow::anyhow!("unsupported scheme `{}`", parsed.scheme())),
        Err(e) => Err(anyhow::anyhow!("`{trimmed}` is not a URL: {e}")),
    }
}

#[path = "pool_link.rs"]
mod pool_link;

#[cfg(test)]
#[path = "pool_tests.rs"]
mod pool_tests;
