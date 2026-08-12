//! Persistent asynchronous MessagePack-RPC client for an embedded Neovim.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use rmpv::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};

use super::notify::NotificationSink;

#[path = "client_io.rs"]
mod client_io;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The stream halves driven by [`NvimClient`].
pub trait Transport: Send + 'static {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    fn split(self) -> (Self::Reader, Self::Writer);
}

impl Transport for (tokio::process::ChildStdout, tokio::process::ChildStdin) {
    type Reader = tokio::process::ChildStdout;
    type Writer = tokio::process::ChildStdin;

    fn split(self) -> (Self::Reader, Self::Writer) {
        self
    }
}

impl Transport for tokio::io::DuplexStream {
    type Reader = tokio::io::ReadHalf<Self>;
    type Writer = tokio::io::WriteHalf<Self>;

    fn split(self) -> (Self::Reader, Self::Writer) {
        tokio::io::split(self)
    }
}

/// Handles a request sent by Neovim to the frontend.
pub trait RequestHandler: Send + Sync + 'static {
    fn request(&self, method: &str, params: &[u8]) -> Result<Value>;
}

struct RejectRequests;

impl RequestHandler for RejectRequests {
    fn request(&self, method: &str, _params: &[u8]) -> Result<Value> {
        bail!("unknown Neovim request `{method}`")
    }
}

type Waiter = oneshot::Sender<Result<Value>>;
type Waiters = Arc<Mutex<HashMap<u32, Waiter>>>;

/// A cloneable connection to one embedded Neovim process.
#[derive(Clone)]
pub struct NvimClient {
    outbox: mpsc::UnboundedSender<Vec<u8>>,
    waiters: Waiters,
    next_id: Arc<AtomicU32>,
    alive: Arc<AtomicBool>,
}

impl NvimClient {
    /// Start the single writer and single reader tasks for a transport.
    pub fn new<T, S>(transport: T, sink: Arc<S>) -> Self
    where
        T: Transport,
        S: NotificationSink,
    {
        Self::with_handler(transport, sink, Arc::new(RejectRequests))
    }

    /// Start a client with application-defined handling for inbound requests.
    pub fn with_handler<T, S, H>(transport: T, sink: Arc<S>, handler: Arc<H>) -> Self
    where
        T: Transport,
        S: NotificationSink,
        H: RequestHandler,
    {
        let (reader, writer) = transport.split();
        let (outbox, inbox) = mpsc::unbounded_channel();
        let client = Self {
            outbox,
            waiters: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU32::new(1)),
            alive: Arc::new(AtomicBool::new(true)),
        };

        tokio::spawn(client_io::write_loop(writer, inbox, client.clone()));
        tokio::spawn(client_io::read_loop(client.clone(), reader, sink, handler));
        client
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Send a request using the normal RPC timeout.
    pub async fn request(&self, method: &str, args: Value) -> Result<Value> {
        self.request_timeout(method, args, DEFAULT_REQUEST_TIMEOUT)
            .await
    }

    /// Send a request and remove its waiter if the timeout expires.
    pub async fn request_timeout(
        &self,
        method: &str,
        args: Value,
        timeout: Duration,
    ) -> Result<Value> {
        let (id, receiver) = self.register_waiter()?;
        let frame = match client_io::encode_request(id, method, args) {
            Ok(frame) => frame,
            Err(error) => {
                self.forget(id);
                return Err(error);
            }
        };
        if let Err(error) = self.send(frame) {
            self.forget(id);
            return Err(error);
        }

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => bail!("Neovim exited while awaiting `{method}`"),
            Err(_) => {
                self.forget(id);
                bail!("Neovim request `{method}` timed out after {timeout:?}")
            }
        }
    }

    /// Queue an already encoded notification. The vector is moved, not copied.
    pub fn notify(&self, bytes: Vec<u8>) -> Result<()> {
        if !self.is_alive() {
            bail!("Neovim connection is closed")
        }
        self.send(bytes)
    }

    fn register_waiter(&self) -> Result<(u32, oneshot::Receiver<Result<Value>>)> {
        let mut waiters = self.lock_waiters();
        if !self.is_alive() {
            bail!("Neovim connection is closed")
        }
        let id = next_id(&self.next_id);
        let (sender, receiver) = oneshot::channel();
        waiters.insert(id, sender);
        Ok((id, receiver))
    }

    fn send(&self, bytes: Vec<u8>) -> Result<()> {
        self.outbox
            .send(bytes)
            .map_err(|_| anyhow!("Neovim connection is closed"))
    }

    fn forget(&self, id: u32) {
        self.lock_waiters().remove(&id);
    }

    fn abandon(&self, reason: impl Into<String>) {
        let mut waiters = self.lock_waiters();
        self.alive.store(false, Ordering::Release);
        let reason = reason.into();
        for (_, sender) in waiters.drain() {
            let _ = sender.send(Err(anyhow!(reason.clone())));
        }
    }

    fn lock_waiters(&self) -> MutexGuard<'_, HashMap<u32, Waiter>> {
        match self.waiters.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn next_id(counter: &AtomicU32) -> u32 {
    loop {
        let id = counter.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod client_tests;
