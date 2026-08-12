use std::str;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use rmpv::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use super::super::frame::{FrameError, Framer, MessageKind, MessageSpan};
use super::super::notify::NotificationSink;
use super::super::scan::{read_array_len, read_uint, skip_value};
use super::super::value::value_to_string;
use super::{NvimClient, RequestHandler};

pub(super) async fn write_loop<W>(
    mut writer: W,
    mut inbox: mpsc::UnboundedReceiver<Vec<u8>>,
    client: NvimClient,
) where
    W: AsyncWrite + Unpin,
{
    while let Some(bytes) = inbox.recv().await {
        if let Err(error) = writer.write_all(&bytes).await {
            client.abandon(format!("Neovim write failed: {error}"));
            return;
        }
        if let Err(error) = writer.flush().await {
            client.abandon(format!("Neovim write failed: {error}"));
            return;
        }
    }
}

pub(super) async fn read_loop<R, S, H>(
    client: NvimClient,
    mut reader: R,
    sink: Arc<S>,
    handler: Arc<H>,
) where
    R: AsyncRead + Unpin,
    S: NotificationSink,
    H: RequestHandler,
{
    let mut framer = Framer::new();
    let mut chunk = [0u8; 8192];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(0) => {
                client.abandon("Neovim exited");
                return;
            }
            Ok(count) => count,
            Err(error) => {
                client.abandon(format!("Neovim read failed: {error}"));
                return;
            }
        };
        framer.push(&chunk[..count]);
        loop {
            let span = match framer.next() {
                Ok(Some(span)) => span,
                Ok(None) => break,
                Err(error) => {
                    client.abandon(frame_error(error));
                    return;
                }
            };
            let bytes = framer.data();
            if let Err(error) = dispatch(&client, &sink, &handler, bytes, &span) {
                client.abandon(format!("Neovim RPC dispatch failed: {error}"));
                return;
            }
        }
    }
}

fn dispatch<S, H>(
    client: &NvimClient,
    sink: &Arc<S>,
    handler: &Arc<H>,
    bytes: &[u8],
    span: &MessageSpan,
) -> Result<()>
where
    S: NotificationSink,
    H: RequestHandler,
{
    let method = str::from_utf8(&bytes[span.method_range.clone()])
        .map_err(|_| anyhow!("invalid UTF-8 method name"))?;
    match span.kind {
        MessageKind::Response => resolve_response(client, bytes, span),
        MessageKind::Notification => {
            sink.notify(method, &bytes[span.params_range.clone()]);
            Ok(())
        }
        MessageKind::Request => reply_to_request(client, handler, method, bytes, span),
    }
}

fn resolve_response(client: &NvimClient, bytes: &[u8], span: &MessageSpan) -> Result<()> {
    let Some(raw_id) = span.msgid else {
        return Ok(());
    };
    let Ok(id) = u32::try_from(raw_id) else {
        return Ok(());
    };
    let waiter = client.lock_waiters().remove(&id);
    let Some(waiter) = waiter else {
        return Ok(());
    };
    let outcome = decode_response(bytes, span);
    let _ = waiter.send(outcome);
    Ok(())
}

fn decode_response(bytes: &[u8], span: &MessageSpan) -> Result<Value> {
    let mut cursor = 0;
    let fields =
        read_array_len(bytes, &mut cursor).map_err(|_| anyhow!("invalid response array"))?;
    let kind = read_uint(bytes, &mut cursor).map_err(|_| anyhow!("invalid response type"))?;
    if fields != 4 || kind != 1 {
        bail!("invalid Neovim response envelope")
    }
    read_uint(bytes, &mut cursor).map_err(|_| anyhow!("invalid response id"))?;
    let error_start = cursor;
    skip_value(bytes, &mut cursor).map_err(|_| anyhow!("invalid response error"))?;
    if cursor != span.params_range.start {
        bail!("invalid Neovim response ranges")
    }
    let mut error_bytes = &bytes[error_start..cursor];
    let error = rmpv::decode::read_value(&mut error_bytes)?;
    if !error_bytes.is_empty() {
        bail!("invalid Neovim response error value")
    }
    if !matches!(error, Value::Nil) {
        let message = value_to_string(&error).unwrap_or_else(|_| "unknown Neovim error".into());
        bail!("Neovim request failed: {message}")
    }
    let mut result_bytes = &bytes[span.params_range.clone()];
    let result = rmpv::decode::read_value(&mut result_bytes)?;
    if !result_bytes.is_empty() {
        bail!("invalid Neovim response result value")
    }
    Ok(result)
}

fn reply_to_request<H>(
    client: &NvimClient,
    handler: &Arc<H>,
    method: &str,
    bytes: &[u8],
    span: &MessageSpan,
) -> Result<()>
where
    H: RequestHandler,
{
    let id = span
        .msgid
        .and_then(|id| u32::try_from(id).ok())
        .ok_or_else(|| anyhow!("invalid Neovim request id"))?;
    let params = &bytes[span.params_range.clone()];
    let response = match handler.request(method, params) {
        Ok(result) => response_frame(id, Value::Nil, result),
        Err(error) => response_frame(id, Value::from(error.to_string()), Value::Nil),
    }?;
    client.send(response)
}

pub(super) fn encode_request(id: u32, method: &str, args: Value) -> Result<Vec<u8>> {
    let frame = Value::Array(vec![
        Value::from(0u64),
        Value::from(u64::from(id)),
        Value::from(method),
        args,
    ]);
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &frame)?;
    Ok(bytes)
}

fn response_frame(id: u32, error: Value, result: Value) -> Result<Vec<u8>> {
    let frame = Value::Array(vec![
        Value::from(1u64),
        Value::from(u64::from(id)),
        error,
        result,
    ]);
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &frame)?;
    Ok(bytes)
}

fn frame_error(error: FrameError) -> String {
    match error {
        FrameError::Malformed => "Neovim sent a malformed RPC frame".into(),
        FrameError::Oversized { length, max } => {
            format!("Neovim RPC frame is {length} bytes, maximum is {max}")
        }
    }
}
