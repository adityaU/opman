/// Core Neovim MessagePack-RPC transport layer.
///
/// Connects to a neovim --listen Unix socket and sends synchronous
/// requests using the msgpack-rpc protocol (type 0 = request, type 1 = response).
use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use rmpv::Value;

#[path = "checked_value.rs"]
mod checked_value;

static MSG_ID: AtomicU32 = AtomicU32::new(1);
const RPC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageType {
    Request,
    Response,
    Notification,
    Other(u64),
}

impl TryFrom<&Value> for MessageType {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self> {
        let number = value
            .as_u64()
            .context("Message type is not an unsigned integer")?;
        Ok(match number {
            0 => Self::Request,
            1 => Self::Response,
            2 => Self::Notification,
            other => Self::Other(other),
        })
    }
}

enum IncomingResponse {
    Skip,
    Matched { error: Value, result: Value },
}

fn parse_response(value: Value, expected_msgid: u64) -> Result<IncomingResponse> {
    let mut fields = match value {
        Value::Array(fields) => fields,
        _ => anyhow::bail!("RPC message is not an array"),
    };
    let message_type = fields
        .first()
        .context("RPC message has no message type")
        .and_then(MessageType::try_from)?;

    match message_type {
        MessageType::Notification => Ok(IncomingResponse::Skip),
        MessageType::Response => {
            if fields.len() < 4 {
                anyhow::bail!("Response array too short: {} fields", fields.len());
            }
            let msgid = fields
                .get(1)
                .and_then(Value::as_u64)
                .context("Response msgid is not an unsigned integer")?;
            if msgid != expected_msgid {
                return Ok(IncomingResponse::Skip);
            }

            // Remove the values from the decoded array instead of cloning the result.
            let result = fields.swap_remove(3);
            let error = fields.swap_remove(2);
            Ok(IncomingResponse::Matched { error, result })
        }
        MessageType::Request | MessageType::Other(_) => {
            anyhow::bail!("Unexpected msgpack-RPC message type: {:?}", message_type)
        }
    }
}

fn read_response(stream: &mut UnixStream, expected_msgid: u64) -> Result<Value> {
    loop {
        let message = rmpv::decode::read_value(&mut *stream)
            .context("Failed to read msgpack response from neovim")?;
        let parsed = parse_response(message, expected_msgid)?;
        let IncomingResponse::Matched { error, result } = parsed else {
            continue;
        };

        if !error.is_nil() {
            let err_msg = match error {
                Value::Array(parts) if parts.len() >= 2 => format!("{}", parts[1]),
                Value::String(_) => format!("{}", error),
                other => format!("{}", other),
            };
            anyhow::bail!("Neovim RPC error: {}", err_msg);
        }
        return Ok(result);
    }
}

fn connect(socket_path: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(socket_path)
        .with_context(|| format!("Failed to connect to neovim at {:?}", socket_path))?;
    stream
        .set_read_timeout(Some(RPC_TIMEOUT))
        .context("Failed to set read timeout")?;
    stream
        .set_write_timeout(Some(RPC_TIMEOUT))
        .context("Failed to set write timeout")?;
    Ok(stream)
}

fn request_bytes(msgid: u32, method: &str, args: Vec<Value>) -> Result<Vec<u8>> {
    let request = Value::Array(vec![
        Value::from(0u64),
        Value::from(msgid as u64),
        Value::from(method),
        Value::Array(args),
    ]);
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, &request).context("Failed to encode msgpack request")?;
    Ok(bytes)
}

fn write_request(stream: &mut UnixStream, request: &[u8]) -> Result<()> {
    stream
        .write_all(request)
        .context("Failed to write to neovim socket")?;
    stream.flush().context("Failed to flush neovim socket")
}

fn call_encoded(socket_path: &Path, msgid: u32, request: &[u8]) -> Result<Value> {
    let mut stream = connect(socket_path)?;
    write_request(&mut stream, request)?;
    read_response(&mut stream, msgid as u64)
}

fn is_read_timeout(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<io::Error>().is_some_and(|io_error| {
            matches!(
                io_error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
        })
    })
}

/// Check for and dismiss a blocking r? prompt.
///
/// This is deliberately called only after a request has timed out. The same
/// stream is used so the prompt can be serviced while the original request
/// remains pending. Every helper response is correlated before it is consumed.
fn dismiss_confirm_prompts(stream: &mut UnixStream) -> Result<()> {
    for _ in 0..10 {
        let mode_msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let mode_request = request_bytes(mode_msgid, "nvim_get_mode", vec![])?;
        write_request(stream, &mode_request)?;
        let mode = read_response(stream, mode_msgid as u64)?;
        let is_confirm = match mode.as_map() {
            Some(pairs) => pairs
                .iter()
                .any(|(key, value)| key.as_str() == Some("mode") && value.as_str() == Some("r?")),
            None => false,
        };

        if !is_confirm {
            return Ok(());
        }

        let input_msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let input_request = request_bytes(input_msgid, "nvim_input", vec![Value::from("E")])?;
        write_request(stream, &input_request)?;
        // Read and validate this response too; it must not leak into a later call.
        let _ = read_response(stream, input_msgid as u64)?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Send an RPC request to neovim and return the result.
pub fn nvim_call(socket_path: &Path, method: &str, args: Vec<Value>) -> Result<Value> {
    let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
    let request = request_bytes(msgid, method, args)?;
    let mut stream = connect(socket_path)?;
    write_request(&mut stream, &request)?;

    match read_response(&mut stream, msgid as u64) {
        Ok(result) => Ok(result),
        Err(error) if is_read_timeout(&error) => {
            dismiss_confirm_prompts(&mut stream)
                .context("Failed to dismiss a blocking neovim prompt")?;
            drop(stream);
            call_encoded(socket_path, msgid, &request)
        }
        Err(error) => Err(error),
    }
}

/// Execute a Vim ex-command (:command).
pub fn nvim_command(socket_path: &Path, cmd: &str) -> Result<()> {
    nvim_call(socket_path, "nvim_command", vec![Value::from(cmd)])?;
    Ok(())
}

/// Feed literal user input into Neovim's normal input queue.
pub fn nvim_input(socket_path: &Path, input: &str) -> Result<i64> {
    let value = nvim_call(socket_path, "nvim_input", vec![Value::from(input)])?;
    value
        .as_i64()
        .context("Neovim returned an invalid input count")
}

/// Execute a Lua expression and return the result.
pub fn nvim_exec_lua(socket_path: &Path, code: &str, args: Vec<Value>) -> Result<Value> {
    nvim_call(
        socket_path,
        "nvim_exec_lua",
        vec![Value::from(code), Value::Array(args)],
    )
}

/// Compatibility wrapper for the existing synchronous nvim_rpc callers.
///
/// New code should use checked_value::ext_or_int directly so malformed Ext
/// values remain an error instead of becoming the current buffer.
pub(crate) fn ext_or_int(value: &Value) -> i64 {
    match checked_value::ext_or_int(value) {
        Ok(handle) => handle,
        Err(_) => i64::MIN,
    }
}

/// Compatibility wrapper for the existing synchronous nvim_rpc callers.
///
/// New code should use checked_value::value_to_string directly so invalid
/// UTF-8 remains an error instead of becoming an empty string.
pub(crate) fn value_to_string(value: &Value) -> String {
    match checked_value::value_to_string(value) {
        Ok(text) => text,
        Err(error) => format!("<invalid msgpack value: {}>", error),
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
