//! One embedded Neovim process and its attached UI channel.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result as AnyResult;
use tokio::process::Child;
use tokio::sync::{broadcast, Mutex, RwLock};

use super::attach;
use super::error::{NvimUiError, Result};
use super::key::{SessionKey, UiSize};
use super::registry_guard::RegistryGuard;
use super::rpc::{NotificationSink, NvimClient};
use super::spawn;
use crate::mcp::NvimSocketRegistry;

impl super::rpc::Transport for tokio::net::UnixStream {
    type Reader = tokio::net::unix::OwnedReadHalf;
    type Writer = tokio::net::unix::OwnedWriteHalf;

    fn split(self) -> (Self::Reader, Self::Writer) {
        self.into_split()
    }
}

const SOCKET_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const NOTIFICATION_CAPACITY: usize = 512;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct NvimNotification {
    pub method: String,
    pub params: Vec<u8>,
}

struct NotificationHub {
    sender: broadcast::Sender<NvimNotification>,
}

impl NotificationHub {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        Self { sender }
    }
}

impl NotificationSink for NotificationHub {
    fn notify(&self, method: &str, params: &[u8]) {
        let _ = self.sender.send(NvimNotification {
            method: method.to_owned(),
            params: params.to_vec(),
        });
    }
}

pub struct NvimSession {
    key: SessionKey,
    project_dir: PathBuf,
    socket_path: PathBuf,
    client: NvimClient,
    child: Mutex<Child>,
    _stdout: tokio::process::ChildStdout,
    _stdin: tokio::process::ChildStdin,
    registry_guard: Arc<RegistryGuard>,
    notifications: Arc<NotificationHub>,
    last_used: AtomicU64,
    size: RwLock<UiSize>,
    dead: AtomicBool,
    shutting_down: AtomicBool,
}

impl NvimSession {
    pub async fn start(
        registry: NvimSocketRegistry,
        key: SessionKey,
        project_dir: &Path,
        size: UiSize,
    ) -> Result<Arc<Self>> {
        Self::start_with_config(
            registry,
            key,
            project_dir,
            size,
            spawn::ConfigSource::UserConfig,
        )
        .await
    }

    pub async fn start_with_config(
        registry: NvimSocketRegistry,
        key: SessionKey,
        project_dir: &Path,
        size: UiSize,
        config: spawn::ConfigSource,
    ) -> Result<Arc<Self>> {
        let spawned = spawn::spawn(project_dir, &key, &config).await?;
        let transport = connect_socket(&spawned.socket_path).await?;
        let notifications = Arc::new(NotificationHub::new());
        let client = NvimClient::new(transport, notifications.clone());
        let registry_guard =
            Arc::new(RegistryGuard::publish(registry, &key, &spawned.socket_path).await?);
        let session = Arc::new(Self {
            key,
            project_dir: project_dir.to_path_buf(),
            socket_path: spawned.socket_path,
            client,
            child: Mutex::new(spawned.child),
            _stdout: spawned.stdout,
            _stdin: spawned.stdin,
            registry_guard,
            notifications,
            last_used: AtomicU64::new(now_millis()),
            size: RwLock::new(size),
            dead: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
        });
        spawn_watchdog(&session);
        if let Err(error) = session.initialize(size).await {
            session.shutdown().await;
            return Err(NvimUiError::Rpc(error));
        }
        Ok(session)
    }

    pub fn key(&self) -> &SessionKey {
        &self.key
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn client(&self) -> NvimClient {
        self.client.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<NvimNotification> {
        self.notifications.sender.subscribe()
    }

    pub fn is_alive(&self) -> bool {
        !self.dead.load(Ordering::Acquire) && self.client.is_alive()
    }

    pub fn touch(&self) {
        self.last_used.store(now_millis(), Ordering::Relaxed);
    }

    pub fn idle_for(&self) -> Duration {
        Duration::from_millis(now_millis().saturating_sub(self.last_used.load(Ordering::Relaxed)))
    }

    pub async fn ui_size(&self) -> UiSize {
        *self.size.read().await
    }

    pub async fn attach(&self) -> Result<()> {
        self.require_alive()?;
        attach::set_client_info(&self.client)
            .await
            .map_err(NvimUiError::Rpc)?;
        let size = self.ui_size().await;
        attach::ui_attach(&self.client, size)
            .await
            .map_err(NvimUiError::Rpc)?;
        self.touch();
        Ok(())
    }

    pub async fn reattach(&self) -> Result<()> {
        self.require_alive()?;
        attach::ui_detach(&self.client)
            .await
            .map_err(NvimUiError::Rpc)?;
        let size = self.ui_size().await;
        attach::ui_attach(&self.client, size)
            .await
            .map_err(NvimUiError::Rpc)?;
        self.touch();
        Ok(())
    }

    pub async fn resize(&self, size: UiSize) -> Result<()> {
        self.require_alive()?;
        attach::ui_try_resize(&self.client, size)
            .await
            .map_err(NvimUiError::Rpc)?;
        *self.size.write().await = size;
        self.touch();
        Ok(())
    }

    pub async fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.client.is_alive() && !self.dead.load(Ordering::Acquire) {
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, attach::ui_detach(&self.client)).await;
        }
        self.mark_dead().await;
        let mut child = self.child.lock().await;
        let _ = child.start_kill();
        let _ = child.wait().await;
    }

    async fn initialize(&self, size: UiSize) -> AnyResult<()> {
        attach::set_client_info(&self.client).await?;
        attach::ui_attach(&self.client, size).await?;
        Ok(())
    }

    async fn mark_dead(&self) {
        if !self.dead.swap(true, Ordering::AcqRel) {
            self.registry_guard.remove().await;
        }
    }

    fn require_alive(&self) -> Result<()> {
        self.is_alive()
            .then_some(())
            .ok_or_else(|| NvimUiError::Rpc(anyhow::anyhow!("Neovim session is dead")))
    }
}

fn spawn_watchdog(session: &Arc<NvimSession>) {
    let weak = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak.upgrade() else {
            return;
        };
        loop {
            let status = {
                let mut child = session.child.lock().await;
                child.try_wait()
            };
            match status {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        session.mark_dead().await;
    });
}

async fn connect_socket(path: &Path) -> Result<tokio::net::UnixStream> {
    let deadline = tokio::time::Instant::now() + SOCKET_CONNECT_TIMEOUT;
    loop {
        match tokio::net::UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(error) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let _ = error;
            }
            Err(error) => return Err(NvimUiError::Spawn(error)),
        }
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;
