//! The browser behind the panes: a local one this process launches, or a device link.
//!
//! A device link is a browser on someone else's machine — the desktop app's own browser,
//! relayed out to this server by [`super::relay`] — attached over
//! `GET /api/browser/device`. While one is attached it takes precedence: panes open in the
//! person's real browser instead of this host's Chromium. The newest link wins; when it
//! drops, the panes go with it (exactly as if a local browser had crashed) and the next
//! pane opens locally again.
//!
//! The *epoch* is what keeps panes honest across those switches: it changes whenever the
//! browser behind the panes does, and the pool drops every pane created under an older one.

use std::path::PathBuf;

use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use super::cdp::Cdp;
use super::chrome::Chrome;
use super::mode::{BrowserMode, Placement};

/// Raw CDP frames buffered each way on a device link.
const LINK_CHANNEL: usize = 256;

/// Identifies one attached device link, so a link that was already replaced cannot detach
/// its successor on the way out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkId(u64);

/// The pool's half of an attached device link; the transport pumps frames through it.
pub struct DeviceLink {
    pub id: LinkId,
    /// Frames the panes send, to be written to the device verbatim.
    pub to_device: mpsc::Receiver<String>,
    /// Frames read from the device, verbatim.
    pub from_device: mpsc::Sender<String>,
    /// Resolves when a newer link replaces this one (or the pool shuts down); the
    /// transport should then close its socket.
    pub superseded: oneshot::Receiver<()>,
}

/// Whether a device link is driving the panes right now. Reported in `/api/browser/list`
/// so the pane can say whose browser it is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    Attached,
    Detached,
}

/// The browser panes are using, as the UI should describe it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct BrowserStatus {
    pub mode: BrowserMode,
    pub device_link: LinkState,
}

/// A CDP client for opening a pane, with where its window goes and the epoch it is
/// valid for.
pub(super) struct Lease {
    pub cdp: Cdp,
    pub placement: Placement,
    pub epoch: u64,
}

struct Running {
    chrome: Chrome,
    cdp: Cdp,
}

struct Link {
    id: LinkId,
    cdp: Cdp,
    /// Dropped to tell the transport it has been replaced.
    _superseded: oneshot::Sender<()>,
}

pub(super) struct Engine {
    mode: BrowserMode,
    /// `None` means the per-user profile for the mode. Overridden by tests, which must not
    /// contend for the profile lock with each other or with a running opman.
    profile: Option<PathBuf>,
    local: Option<Running>,
    link: Option<Link>,
    epoch: u64,
    next_link: u64,
}

impl Engine {
    pub fn new(mode: BrowserMode, profile: Option<PathBuf>) -> Self {
        Self {
            mode,
            profile,
            local: None,
            link: None,
            epoch: 0,
            next_link: 0,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn status(&self) -> BrowserStatus {
        BrowserStatus {
            mode: self.mode,
            device_link: match self.link {
                Some(_) => LinkState::Attached,
                None => LinkState::Detached,
            },
        }
    }

    /// The client panes should open on: the device link if one is attached, otherwise the
    /// local browser, launched if needed.
    pub async fn lease(&mut self) -> anyhow::Result<Lease> {
        if let Some(link) = &self.link {
            return Ok(Lease {
                cdp: link.cdp.clone(),
                // Someone's real desktop: never put a pane window in front of them.
                placement: Placement::OffScreen,
                epoch: self.epoch,
            });
        }
        let cdp = self.local().await?.cdp.clone();
        Ok(Lease {
            cdp,
            placement: self.mode.placement(),
            epoch: self.epoch,
        })
    }

    /// The local browser's DevTools endpoint, launching it if needed. What a relay dials.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn local_endpoint(&mut self) -> anyhow::Result<String> {
        Ok(self.local().await?.chrome.ws_url().to_owned())
    }

    /// The local browser, relaunched if it died. A dead one bumps the epoch only when panes
    /// were on it — with a link attached, they are not.
    async fn local(&mut self) -> anyhow::Result<&Running> {
        if let Some(running) = self.local.as_mut() {
            if !running.chrome.is_alive() {
                self.local = None;
                if self.link.is_none() {
                    self.epoch += 1;
                }
            }
        }
        if self.local.is_none() {
            let dir = self.profile_dir()?;
            let chrome = Chrome::launch(&dir, self.mode).await?;
            let cdp = Cdp::connect(chrome.ws_url()).await?;
            self.local = Some(Running { chrome, cdp });
        }
        self.local
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("the browser is not running"))
    }

    fn profile_dir(&self) -> anyhow::Result<PathBuf> {
        let dir = match &self.profile {
            Some(dir) => dir.clone(),
            None => dirs::data_dir()
                .ok_or_else(|| anyhow::anyhow!("no data directory on this platform"))?
                .join("opman")
                .join(self.mode.profile_dir_name()),
        };
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Make a new device link the browser behind the panes, replacing any previous one.
    pub fn attach_link(&mut self) -> DeviceLink {
        let (outgoing, to_device) = mpsc::channel(LINK_CHANNEL);
        let (from_device, incoming) = mpsc::channel(LINK_CHANNEL);
        let (superseded_tx, superseded) = oneshot::channel();
        let id = LinkId(self.next_link);
        self.next_link += 1;
        self.epoch += 1;
        // Replacing drops the old link's sender, which is its transport's cue to close.
        self.link = Some(Link {
            id,
            cdp: Cdp::from_channels(outgoing, incoming),
            _superseded: superseded_tx,
        });
        DeviceLink {
            id,
            to_device,
            from_device,
            superseded,
        }
    }

    /// Forget a link whose transport ended. `Detach::Ignored` when it had already been
    /// replaced — its successor's panes are not its to drop.
    pub fn detach_link(&mut self, id: LinkId) -> Detach {
        if self.link.as_ref().map(|link| link.id) != Some(id) {
            return Detach::Ignored;
        }
        self.link = None;
        self.epoch += 1;
        Detach::Detached
    }

    /// Stop everything this process started. The link is dropped (its transport closes);
    /// a launched browser is shut down, an adopted one left running.
    // Reached only through `BrowserPool::shutdown`, which nothing calls yet.
    #[allow(dead_code)]
    pub async fn shutdown(&mut self) {
        self.link = None;
        if let Some(running) = self.local.take() {
            running.chrome.shutdown().await;
        }
    }
}

/// What [`Engine::detach_link`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detach {
    Detached,
    Ignored,
}
