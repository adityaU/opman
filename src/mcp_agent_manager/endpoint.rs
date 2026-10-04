//! The self-healing endpoint used by the in-process agent manager.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::ipc::{EndpointId, IpcListener};

/// How often the endpoint checks that its pathname still names its own listener.
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

/// The endpoint's complete ownership: pathname, bind-time identity, and listener.
pub(crate) struct EndpointParts {
    identity: EndpointIdentity,
    listener: IpcListener,
}

pub(crate) struct EndpointIdentity {
    path: PathBuf,
    id: EndpointId,
}

impl Endpoint {
    pub(crate) fn bind(path: PathBuf) -> Result<Self> {
        let parts = EndpointParts::bind(path)?;
        Ok(Self {
            state: EndpointState::Bound(parts),
        })
    }

    pub(crate) fn from_parts(identity: EndpointIdentity, listener: IpcListener) -> Self {
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
            expected = ?parts.identity.id,
            actual = ?EndpointId::of(&parts.identity.path).ok(),
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
        match EndpointParts::bind(self.parts.identity.path.clone()) {
            Ok(parts) => (
                Endpoint {
                    state: EndpointState::Bound(parts),
                },
                Ok(()),
            ),
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
    fn bind(path: PathBuf) -> Result<Self> {
        let listener = IpcListener::bind(&path).with_context(|| {
            format!("failed to bind agent manager socket at {}", path.display())
        })?;
        let id = EndpointId::of(&path).with_context(|| {
            format!("failed to stat agent manager socket at {}", path.display())
        })?;
        Ok(Self {
            identity: EndpointIdentity { path, id },
            listener,
        })
    }

    pub(crate) fn split(self) -> (EndpointIdentity, IpcListener) {
        (self.identity, self.listener)
    }

    fn is_current(&self) -> bool {
        EndpointId::of(&self.identity.path).ok() == Some(self.identity.id)
    }
}

impl Drop for EndpointIdentity {
    fn drop(&mut self) {
        if let Err(error) = self.id.release(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                %error,
                "failed to remove agent manager socket during shutdown"
            );
        }
    }
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod endpoint_tests;
