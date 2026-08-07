//! `Content-Length` framing — the wire format every language server speaks.
//!
//! LSP frames are an HTTP-style header block, a blank line, then exactly
//! `Content-Length` bytes of JSON. Unlike the newline-delimited ACP transport
//! in [`crate::acp_engine::jsonrpc`], there is no resync point: a byte count we
//! cannot trust means every subsequent frame boundary is guesswork. So a
//! malformed header is a hard error that tears the connection down and lets the
//! pool respawn, rather than something to skip past.
//!
//! Everything here is generic over `AsyncRead`/`AsyncWrite` so tests can drive
//! a scripted server over `tokio::io::duplex()` with no process involved.

use anyhow::{bail, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Refuse absurd frames rather than trying to allocate them. Real LSP payloads
/// (rust-analyzer's initialize result, a big publishDiagnostics) stay well under
/// this; anything larger means the stream is desynchronised.
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Read one frame. `Ok(None)` is a clean EOF — the server exited between frames.
pub async fn read_frame<R>(reader: &mut R) -> Result<Option<Value>>
where
    R: AsyncBufReadExt + Unpin,
{
    let Some(len) = read_headers(reader).await? else {
        return Ok(None);
    };
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(Some(serde_json::from_slice(&body)?))
}

/// Consume the header block, returning the declared body length.
/// `Ok(None)` when the stream ends before any header byte arrives.
async fn read_headers<R>(reader: &mut R) -> Result<Option<usize>>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut content_length: Option<usize> = None;
    let mut saw_any = false;
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            if saw_any {
                bail!("language server closed the stream mid-header");
            }
            return Ok(None);
        }
        saw_any = true;

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        let Some((name, value)) = trimmed.split_once(':') else {
            bail!("malformed LSP header line: {trimmed:?}");
        };
        if !name.eq_ignore_ascii_case("content-length") {
            continue; // Content-Type is the only other header, and it is advisory.
        }
        content_length = Some(value.trim().parse()?);
    }

    let Some(len) = content_length else {
        bail!("LSP frame has no Content-Length header");
    };
    if len > MAX_FRAME_BYTES {
        bail!("LSP frame of {len} bytes exceeds the {MAX_FRAME_BYTES} byte limit");
    }
    Ok(Some(len))
}

/// Write one frame, header and all, and flush it.
pub async fn write_frame<W>(writer: &mut W, frame: &Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(frame)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// Marker for the halves of a duplex stream a [`crate::lsp::peer::Peer`] drives.
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

#[cfg(test)]
#[path = "framing_tests.rs"]
mod framing_tests;
