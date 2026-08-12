//! The self-healing Unix endpoint used by the in-process agent manager.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::UnixListener;

/// How often the endpoint checks that its pathname still names its own inode.
pub(crate) const SUPERVISOR_INTERVAL: Duration = Duration::from_secs(1);

/// Stable per-process path used by opman and every child that reaches the endpoint.
pub fn socket_path() -> PathBuf {
    let directory = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(directory) if !directory.is_empty() => PathBuf::from(directory),
        _ => {
            tracing::error!(
                "XDG_RUNTIME_DIR is unset; placing the agent-manager socket in the weaker temp directory"
            );
            std::env::temp_dir()
        }
    };
    directory.join(format!("opman-agent-manager-{}.sock", std::process::id()))
}

/// An endpoint is either still bound to its captured inode or orphaned from its pathname.
pub(crate) struct Endpoint {
    state: EndpointState,
}

pub(crate) fn supervise(endpoint: Endpoint) -> Endpoint {
    let (endpoint, result) = endpoint.tick();
    if let Err(error) = result {
        tracing::error!(%error, "agent manager socket supervision failed; will retry");
    }
    endpoint
}

enum EndpointState {
    Bound(EndpointParts),
    Orphaned(EndpointParts),
}

/// The endpoint's complete ownership: pathname, bind-time inode, and listener.
pub(crate) struct EndpointParts {
    identity: EndpointIdentity,
    listener: UnixListener,
}

pub(crate) struct EndpointIdentity {
    path: PathBuf,
    inode: u64,
}

impl Endpoint {
    pub(crate) fn bind(path: PathBuf) -> Result<Self> {
        let listener = bind_listener(&path)?;
        let inode = fs::metadata(&path)
            .with_context(|| format!("failed to stat agent manager socket at {}", path.display()))?
            .ino();
        Ok(Self {
            state: EndpointState::Bound(EndpointParts {
                identity: EndpointIdentity { path, inode },
                listener,
            }),
        })
    }

    pub(crate) fn from_parts(identity: EndpointIdentity, listener: UnixListener) -> Self {
        Self {
            state: EndpointState::Bound(EndpointParts { identity, listener }),
        }
    }

    pub(crate) fn is_bound(&self) -> bool {
        matches!(self.state, EndpointState::Bound(_))
    }

    pub(crate) fn take_bound(self) -> Option<EndpointParts> {
        match self.state {
            EndpointState::Bound(parts) => Some(parts),
            EndpointState::Orphaned(parts) => {
                drop(parts);
                None
            }
        }
    }

    /// Check the path and, if necessary, replace the listener with a fresh bind.
    ///
    /// The endpoint is returned even when rebinding fails, so the supervisor can retry on its
    /// next tick while retaining ownership of the old listener and its cleanup identity.
    pub(crate) fn tick(self) -> (Self, Result<()>) {
        match self.state {
            EndpointState::Bound(parts) if parts.is_current() => (
                Self {
                    state: EndpointState::Bound(parts),
                },
                Ok(()),
            ),
            EndpointState::Bound(parts) => Self::orphaned(parts).rebind(),
            EndpointState::Orphaned(parts) => Self::orphaned(parts).rebind(),
        }
    }

    fn orphaned(parts: EndpointParts) -> OrphanedEndpoint {
        tracing::error!(
            path = %parts.identity.path.display(),
            expected_inode = parts.identity.inode,
            actual_inode = ?parts.current_inode(),
            "agent manager socket path no longer names this listener"
        );
        OrphanedEndpoint { parts }
    }
}

struct OrphanedEndpoint {
    parts: EndpointParts,
}

impl OrphanedEndpoint {
    fn rebind(self) -> (Endpoint, Result<()>) {
        let path = self.parts.identity.path.clone();
        match bind_listener(&path) {
            Ok(listener) => {
                let inode = match fs::metadata(&path) {
                    Ok(metadata) => metadata.ino(),
                    Err(error) => {
                        return (
                            Endpoint {
                                state: EndpointState::Orphaned(self.parts),
                            },
                            Err(error).with_context(|| {
                                format!(
                                    "failed to stat rebound agent manager socket at {}",
                                    path.display()
                                )
                            }),
                        );
                    }
                };
                let replacement = Endpoint {
                    state: EndpointState::Bound(EndpointParts {
                        identity: EndpointIdentity { path, inode },
                        listener,
                    }),
                };
                (replacement, Ok(()))
            }
            Err(error) => (
                Endpoint {
                    state: EndpointState::Orphaned(self.parts),
                },
                Err(error),
            ),
        }
    }
}

impl EndpointParts {
    pub(crate) fn split(self) -> (EndpointIdentity, UnixListener) {
        (self.identity, self.listener)
    }

    fn is_current(&self) -> bool {
        self.current_inode() == Some(self.identity.inode)
    }

    fn current_inode(&self) -> Option<u64> {
        fs::metadata(&self.identity.path)
            .ok()
            .map(|metadata| metadata.ino())
    }
}

impl Drop for EndpointIdentity {
    fn drop(&mut self) {
        let owned = fs::metadata(&self.path)
            .ok()
            .is_some_and(|metadata| metadata.ino() == self.inode);
        if owned {
            if let Err(error) = fs::remove_file(&self.path) {
                tracing::warn!(
                    path = %self.path.display(),
                    %error,
                    "failed to remove agent manager socket during shutdown"
                );
            }
        }
    }
}

fn bind_listener(path: &Path) -> Result<UnixListener> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to remove stale agent manager socket at {}",
                    path.display()
                )
            });
        }
    }
    let listener = std::os::unix::net::UnixListener::bind(path)
        .with_context(|| format!("failed to bind agent manager socket at {}", path.display()))?;
    listener
        .set_nonblocking(true)
        .context("failed to configure agent manager socket")?;
    UnixListener::from_std(listener).context("failed to initialize agent manager socket")
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod endpoint_tests;
