//! Local IPC between opman and its own child processes.
//!
//! Callers name an endpoint by a filesystem path. On Unix that path is a domain
//! socket; on Windows it maps to a named pipe, which lives outside the
//! filesystem. Everything else — framing, half-close, reconnect — is identical, so
//! no caller needs a `cfg` of its own.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{blocking, connect, EndpointId, IpcListener, IpcStream, IpcWriteHalf};
#[cfg(windows)]
pub use windows::{blocking, connect, EndpointId, IpcListener, IpcStream, IpcWriteHalf};
