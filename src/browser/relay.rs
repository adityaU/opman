//! Home side of a device link: lend this device's browser to a remote opman.
//!
//! In device mode the desktop app's opman (home) runs the person's real browser. A remote
//! server — a dev box — cannot reach it, so home dials out instead: it opens a websocket
//! to the remote's `GET /api/browser/device`, opens a *fresh* DevTools connection to the
//! local browser, and copies frames between the two verbatim. The remote's panes then
//! open in this browser (see [`super::engine`]), and a relay never has to understand CDP
//! beyond remembering which windows it should close when the link ends.
//!
//! [`run_device_link`] serves one connection; [`run_device_link_loop`] keeps one alive
//! with backoff until told to stop.

// Unreferenced until the remote-server supervisor starts a relay per configured remote.

use std::future::Future;
use std::time::{Duration, Instant};

use futures::{Sink, Stream};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use super::mode::BrowserMode;
use super::pool::BrowserPool;

/// First retry delay; doubled after each failure up to [`MAX_BACKOFF`].
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// A link that stayed up this long counts as healthy: the next retry starts fast again.
const STABLE_LINK: Duration = Duration::from_secs(10);

/// How a relay ended without an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayEnd {
    /// The remote closed the link (or replaced it with a newer one).
    UpstreamClosed,
    /// The local browser closed its DevTools connection — it quit or crashed.
    BrowserClosed,
}

#[derive(Debug)]
pub enum RelayError {
    /// Only a device-mode opman has a device browser to lend.
    NotDeviceMode,
    /// The local browser could not be launched or reached.
    Browser(anyhow::Error),
    /// Dialling the remote failed (from the caller's connect closure).
    Upstream(anyhow::Error),
    /// A websocket failed mid-relay.
    Socket(WsError),
}

impl std::fmt::Display for RelayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotDeviceMode => {
                f.write_str("the device browser relay needs opman --device-browser")
            }
            Self::Browser(error) => write!(f, "device browser unavailable: {error:#}"),
            Self::Upstream(error) => write!(f, "could not reach the remote server: {error:#}"),
            Self::Socket(error) => write!(f, "device link failed: {error}"),
        }
    }
}

impl std::error::Error for RelayError {}

/// Relay one connected link until either side closes.
///
/// `upstream` is a websocket already connected (and authenticated) to a remote's
/// `/api/browser/device` — typically the stream from `tokio_tungstenite::connect_async`.
/// Windows the remote created through this relay are closed when it ends, because the
/// remote forgets its panes the moment the link drops and nobody else could close them.
pub async fn run_device_link<S>(pool: &BrowserPool, upstream: S) -> Result<RelayEnd, RelayError>
where
    S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
{
    if pool.mode() != BrowserMode::Device {
        return Err(RelayError::NotDeviceMode);
    }
    let endpoint = pool.local_endpoint().await.map_err(RelayError::Browser)?;
    let (local, _) = tokio_tungstenite::connect_async(endpoint.as_str())
        .await
        .map_err(|error| RelayError::Browser(error.into()))?;
    super::relay_pipe::pipe(upstream, local).await
}

/// Keep a device link to one remote alive until `shutdown` resolves.
///
/// `connect` dials the remote and returns the connected websocket (the caller owns the
/// URL and the auth). Failures and closed links are retried after 1s, doubling to 30s; a
/// link that stayed up for a while resets the delay. Returns `Ok(())` on shutdown, or
/// [`RelayError::NotDeviceMode`] at once when there is no device browser to lend.
pub async fn run_device_link_loop<C, Fut, S, Shutdown>(
    pool: BrowserPool,
    mut connect: C,
    shutdown: Shutdown,
) -> Result<(), RelayError>
where
    C: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<S>>,
    S: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
    Shutdown: Future<Output = ()>,
{
    if pool.mode() != BrowserMode::Device {
        return Err(RelayError::NotDeviceMode);
    }
    tokio::pin!(shutdown);
    let mut backoff = Backoff::default();
    loop {
        let attempt = async {
            let upstream = connect().await.map_err(RelayError::Upstream)?;
            let started = Instant::now();
            let ended = run_device_link(&pool, upstream).await;
            Ok::<_, RelayError>((ended, started.elapsed()))
        };
        let outcome = tokio::select! {
            () = &mut shutdown => return Ok(()),
            outcome = attempt => outcome,
        };
        match outcome {
            Ok((Err(RelayError::NotDeviceMode), _)) => return Err(RelayError::NotDeviceMode),
            Ok((ended, lasted)) => {
                match ended {
                    Ok(end) => tracing::info!(?end, "device link ended"),
                    Err(error) => tracing::warn!(%error, "device link failed"),
                }
                if lasted >= STABLE_LINK {
                    backoff.reset();
                }
            }
            Err(error) => tracing::debug!(%error, "device link could not connect"),
        }
        let delay = backoff.next_delay();
        tokio::select! {
            () = &mut shutdown => return Ok(()),
            () = tokio::time::sleep(delay) => {}
        }
    }
}

/// Exponential retry delay: 1s, 2s, 4s … capped at 30s.
#[derive(Debug)]
struct Backoff {
    next: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { next: MIN_BACKOFF }
    }
}

impl Backoff {
    fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(MAX_BACKOFF);
        delay
    }

    fn reset(&mut self) {
        self.next = MIN_BACKOFF;
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod relay_tests;
