//! The live remote server list: CRUD, status probes, and the handles other subsystems use.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{watch, Mutex, RwLock};

use super::session::Remote;
use super::store::ServerStore;
use super::types::{
    Credentials, RemoteError, ServerId, ServerRecord, ServerSnapshot, ServerStatus, ServerUrl,
};
use super::ws::{self, RemoteWs};
use crate::mcp_oauth::Secret;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// A server to add. Credentials arrive in plain text from the settings page and go
/// straight into the store.
pub struct NewServer {
    pub name: String,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// A partial edit: `None` leaves a field alone. An empty `username` removes auth.
#[derive(Default)]
pub struct ServerPatch {
    pub name: Option<String>,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Shared, cheaply cloneable handle on the remote server list.
#[derive(Clone)]
pub struct Registry {
    inner: Arc<Inner>,
}

struct Inner {
    store: ServerStore,
    /// No overall timeout: proxied SSE streams stay open indefinitely.
    http: reqwest::Client,
    remotes: RwLock<Vec<Arc<Remote>>>,
    /// Serialises edits so two adds cannot claim one id or lose each other's write.
    edits: Mutex<()>,
    changes: watch::Sender<u64>,
}

impl Registry {
    pub fn new(store: ServerStore) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let remotes = store
            .load()
            .into_iter()
            .map(|record| Arc::new(Remote::new(record)))
            .collect();
        Self {
            inner: Arc::new(Inner {
                store,
                http,
                remotes: RwLock::new(remotes),
                edits: Mutex::new(()),
                changes: watch::channel(0).0,
            }),
        }
    }

    /// The user's `~/.config/opman/servers.json`.
    pub fn load() -> Self {
        Self::new(ServerStore::open().unwrap_or_else(ServerStore::memory))
    }

    /// Every configured remote, in the order the user added them. No credentials.
    // Unused until the device-browser relay is wired; drop the allow then.
    pub async fn servers(&self) -> Vec<ServerSnapshot> {
        let remotes = self.inner.remotes.read().await;
        remotes.iter().map(|r| ServerSnapshot::from(&r.record)).collect()
    }

    /// Ticks (the value is a generation counter) whenever a server is added, edited or
    /// removed. Re-read [`Registry::servers`] on each change.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.changes.subscribe()
    }

    /// An authenticated WebSocket to `{remote}{path}`, e.g. `path = "/api/browser/device"`.
    pub async fn connect_ws(&self, id: &ServerId, path: &str) -> Result<RemoteWs, RemoteError> {
        let remote = self.remote(id).await.ok_or(RemoteError::UnknownServer)?;
        ws::connect(&remote, &self.inner.http, path).await
    }

    pub(crate) async fn remote(&self, id: &ServerId) -> Option<Arc<Remote>> {
        let remotes = self.inner.remotes.read().await;
        remotes.iter().find(|r| &r.record.id == id).cloned()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.inner.http
    }

    /// Probe every remote concurrently, each bounded by the check timeout.
    pub async fn statuses(&self) -> Vec<(ServerSnapshot, ServerStatus)> {
        let remotes = self.inner.remotes.read().await.clone();
        let http = &self.inner.http;
        let probes = remotes.iter().map(|remote| async move {
            (ServerSnapshot::from(&remote.record), remote.probe(http).await)
        });
        futures::future::join_all(probes).await
    }

    /// Validate by logging in, then persist. Nothing is stored if the remote refuses.
    pub async fn add(&self, new: NewServer) -> Result<ServerSnapshot, RemoteError> {
        let name = clean_name(&new.name)?;
        let url = ServerUrl::parse(&new.url)?;
        let credentials = credentials(new.username, new.password.map(Secret::new), None);
        let _edit = self.inner.edits.lock().await;
        let id = {
            let remotes = self.inner.remotes.read().await;
            ServerId::derive(&name, |id| remotes.iter().any(|r| &r.record.id == id))
        };
        let remote = Remote::new(ServerRecord {
            id,
            name,
            url,
            credentials,
        });
        remote.check(&self.inner.http).await?;
        let snapshot = ServerSnapshot::from(&remote.record);
        self.commit(|remotes| remotes.push(Arc::new(remote))).await?;
        Ok(snapshot)
    }

    /// Apply `patch`. A change to the URL or credentials is validated like an add; a
    /// rename alone is saved as-is and reported with a fresh probe.
    pub async fn update(
        &self,
        id: &ServerId,
        patch: ServerPatch,
    ) -> Result<(ServerSnapshot, ServerStatus), RemoteError> {
        let _edit = self.inner.edits.lock().await;
        let current = self.remote(id).await.ok_or(RemoteError::UnknownServer)?;
        let old = &current.record;
        let reconnect = patch.url.is_some() || patch.username.is_some() || patch.password.is_some();
        let record = ServerRecord {
            id: old.id.clone(),
            name: match patch.name {
                Some(name) => clean_name(&name)?,
                None => old.name.clone(),
            },
            url: match patch.url {
                Some(url) => ServerUrl::parse(&url)?,
                None => old.url.clone(),
            },
            credentials: credentials(
                patch.username,
                patch.password.map(Secret::new),
                Some(&old.credentials),
            ),
        };
        let remote = Remote::new(record);
        let status = if reconnect {
            remote.check(&self.inner.http).await?;
            ServerStatus::Ok
        } else {
            remote.probe(&self.inner.http).await
        };
        let snapshot = ServerSnapshot::from(&remote.record);
        let remote = Arc::new(remote);
        self.commit(|remotes| {
            if let Some(slot) = remotes.iter_mut().find(|r| &r.record.id == id) {
                *slot = remote;
            }
        })
        .await?;
        Ok((snapshot, status))
    }

    pub async fn remove(&self, id: &ServerId) -> Result<(), RemoteError> {
        let _edit = self.inner.edits.lock().await;
        if self.remote(id).await.is_none() {
            return Err(RemoteError::UnknownServer);
        }
        self.commit(|remotes| remotes.retain(|r| &r.record.id != id))
            .await
    }

    /// Edit a copy, persist it, and only then publish it, so a failed write changes
    /// nothing. Callers hold `edits`.
    async fn commit(&self, edit: impl FnOnce(&mut Vec<Arc<Remote>>)) -> Result<(), RemoteError> {
        let mut next = self.inner.remotes.read().await.clone();
        edit(&mut next);
        let records: Vec<ServerRecord> = next.iter().map(|r| r.record.clone()).collect();
        self.inner.store.save(&records)?;
        *self.inner.remotes.write().await = next;
        self.inner.changes.send_modify(|generation| *generation += 1);
        Ok(())
    }
}

fn clean_name(raw: &str) -> Result<String, RemoteError> {
    match raw.trim() {
        "" => Err(RemoteError::InvalidName),
        name => Ok(name.to_owned()),
    }
}

/// Merge supplied credentials over `previous`. An absent password keeps the stored one.
fn credentials(
    username: Option<String>,
    password: Option<Secret>,
    previous: Option<&Credentials>,
) -> Credentials {
    let (stored_user, stored_password) = match previous {
        Some(Credentials::Password { username, password }) => (Some(username), Some(password)),
        _ => (None, None),
    };
    let username = match username {
        Some(name) => name.trim().to_owned(),
        None => match stored_user {
            Some(name) => name.clone(),
            None => return Credentials::Open,
        },
    };
    if username.is_empty() {
        return Credentials::Open;
    }
    let password = password
        .or_else(|| stored_password.cloned())
        .unwrap_or_else(|| Secret::new(""));
    Credentials::Password { username, password }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod registry_tests;
