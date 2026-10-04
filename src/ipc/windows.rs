//! Named pipes. Each endpoint path maps to one pipe name; the pipe namespace is
//! not a filesystem, so nothing can unlink an endpoint out from under its listener.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

/// `ERROR_PIPE_BUSY`: every server instance is mid-handshake; retry shortly.
const ERROR_PIPE_BUSY: i32 = 231;
const BUSY_RETRY: Duration = Duration::from_millis(20);
const BUSY_ATTEMPTS: u32 = 100;

/// `\\.\pipe\` plus the path. Backslash is the only character a pipe name forbids.
fn pipe_name(path: &Path) -> OsString {
    let mut name = OsString::from(r"\\.\pipe\");
    name.push(path.to_string_lossy().replace('\\', "/"));
    name
}

pub enum IpcStream {
    Server(NamedPipeServer),
    Client(NamedPipeClient),
}

pub type IpcWriteHalf = WriteHalf<IpcStream>;

impl IpcStream {
    pub fn into_split(self) -> (ReadHalf<Self>, WriteHalf<Self>) {
        tokio::io::split(self)
    }
}

impl AsyncRead for IpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_read(cx, buf),
            Self::Client(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_write(cx, buf),
            Self::Client(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_flush(cx),
            Self::Client(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_shutdown(cx),
            Self::Client(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
}

pub async fn connect(path: &Path) -> io::Result<IpcStream> {
    let name = pipe_name(path);
    let mut attempts = 0;
    loop {
        match ClientOptions::new().open(&name) {
            Ok(client) => return Ok(IpcStream::Client(client)),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                attempts += 1;
                if attempts == BUSY_ATTEMPTS {
                    return Err(error);
                }
                tokio::time::sleep(BUSY_RETRY).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// A pipe server always keeps one idle instance waiting, so a client never
/// sees the pipe missing between two accepts.
pub struct IpcListener {
    name: OsString,
    waiting: NamedPipeServer,
}

impl IpcListener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        let name = pipe_name(path);
        let waiting = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&name)?;
        Ok(Self { name, waiting })
    }

    pub async fn accept(&mut self) -> io::Result<IpcStream> {
        self.waiting.connect().await?;
        let next = ServerOptions::new().create(&self.name)?;
        Ok(IpcStream::Server(std::mem::replace(
            &mut self.waiting,
            next,
        )))
    }
}

/// Pipes cannot be unlinked or replaced by another process, so every endpoint
/// stays current for as long as its listener lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointId;

impl EndpointId {
    pub fn of(_path: &Path) -> io::Result<Self> {
        Ok(Self)
    }

    pub fn release(self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// Synchronous clients are only used for Neovim's msgpack RPC, which needs read
/// deadlines that Windows pipes opened as files cannot provide.
pub mod blocking {
    use std::io;
    use std::path::Path;
    use std::time::Duration;

    /// No value of this type can exist: `connect` always fails on Windows.
    pub enum Stream {}

    pub fn connect(_path: &Path, _timeout: Duration) -> io::Result<Stream> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "synchronous socket clients are not supported on Windows",
        ))
    }

    impl io::Read for Stream {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            match *self {}
        }
    }

    impl io::Write for Stream {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            match *self {}
        }

        fn flush(&mut self) -> io::Result<()> {
            match *self {}
        }
    }
}
