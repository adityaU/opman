//! Unix domain sockets. The types are tokio's own, so this costs nothing over
//! using them directly.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub type IpcStream = tokio::net::UnixStream;
pub type IpcWriteHalf = tokio::net::unix::OwnedWriteHalf;

pub async fn connect(path: &Path) -> io::Result<IpcStream> {
    IpcStream::connect(path).await
}

/// A bound endpoint. Binding replaces any stale socket file left at the path.
pub struct IpcListener(tokio::net::UnixListener);

impl IpcListener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = std::os::unix::net::UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        tokio::net::UnixListener::from_std(listener).map(Self)
    }

    pub async fn accept(&mut self) -> io::Result<IpcStream> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}

/// What an endpoint path currently points at: the socket file's inode. Another
/// process can unlink or replace the file, which orphans a live listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointId(u64);

impl EndpointId {
    pub fn of(path: &Path) -> io::Result<Self> {
        std::fs::metadata(path).map(|metadata| Self(metadata.ino()))
    }

    /// Remove the socket file, but only while it still belongs to this endpoint.
    pub fn release(self, path: &Path) -> io::Result<()> {
        if Self::of(path).ok() != Some(self) {
            return Ok(());
        }
        std::fs::remove_file(path)
    }
}

/// Synchronous client with read and write deadlines.
pub mod blocking {
    use std::io;
    use std::path::Path;
    use std::time::Duration;

    pub type Stream = std::os::unix::net::UnixStream;

    pub fn connect(path: &Path, timeout: Duration) -> io::Result<Stream> {
        let stream = Stream::connect(path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(stream)
    }
}
