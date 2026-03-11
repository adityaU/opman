/// Core Neovim MessagePack-RPC transport layer.
///
/// Connects to a neovim `--listen` Unix socket and sends synchronous
/// requests using the msgpack-rpc protocol (type 0 = request, type 1 = response).
///
/// Protocol format (msgpack array):
///   Request:  [0, msgid, method, params]
///   Response: [1, msgid, error, result]
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use rmpv::Value;

/// Global message ID counter (monotonically increasing).
static MSG_ID: AtomicU32 = AtomicU32::new(1);

/// Check if neovim is in `r?` (confirm) mode and auto-dismiss the prompt.
///
/// When neovim encounters a swap file, file-changed-on-disk, or similar
/// situation it enters `r?` mode — a blocking confirmation dialog that
/// prevents any further RPC calls from completing. This function detects
/// that state and sends `E` (Edit anyway) to dismiss it, looping until
/// the mode clears.
fn dismiss_confirm_prompts(stream: &mut UnixStream) -> Result<()> {
    // Try up to 10 times in case prompts cascade (e.g. multiple swap files)
    for _ in 0..10 {
        let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let mode_request = Value::Array(vec![
            Value::from(0u64),
            Value::from(msgid as u64),
            Value::from("nvim_get_mode"),
            Value::Array(vec![]),
        ]);

        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &mode_request)
            .context("Failed to encode mode request")?;
        stream
            .write_all(&buf)
            .context("Failed to write mode request")?;
        stream.flush()?;

        let response =
            rmpv::decode::read_value(&mut *stream).context("Failed to read mode response")?;

        // Parse: [1, msgid, nil, {mode: "...", blocking: bool}]
        let is_confirm = response
            .as_array()
            .and_then(|arr| arr.get(3))
            .and_then(|result| result.as_map())
            .map(|pairs| {
                pairs
                    .iter()
                    .any(|(k, v)| k.as_str() == Some("mode") && v.as_str() == Some("r?"))
            })
            .unwrap_or(false);

        if !is_confirm {
            return Ok(());
        }

        // Dismiss the prompt by sending 'E' (Edit anyway)
        let input_msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let input_request = Value::Array(vec![
            Value::from(0u64),
            Value::from(input_msgid as u64),
            Value::from("nvim_input"),
            Value::Array(vec![Value::from("E")]),
        ]);

        let mut input_buf = Vec::new();
        rmpv::encode::write_value(&mut input_buf, &input_request)
            .context("Failed to encode input request")?;
        stream
            .write_all(&input_buf)
            .context("Failed to write input request")?;
        stream.flush()?;

        // Read and discard the nvim_input response
        let _ = rmpv::decode::read_value(&mut *stream);

        // Give neovim time to process before checking again
        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Send an RPC request to neovim and return the result.
///
/// Connects fresh for each call (neovim's socket supports multiple connections).
/// Timeout prevents hanging if neovim is unresponsive.
/// Automatically dismisses any confirm-mode prompts before the actual call.
pub fn nvim_call(socket_path: &Path, method: &str, args: Vec<Value>) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("Failed to connect to neovim at {:?}", socket_path))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("Failed to set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("Failed to set write timeout")?;

    // Auto-dismiss any confirm prompts (swap file dialogs, etc.)
    dismiss_confirm_prompts(&mut stream)?;

    let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);

    // Encode request: [0, msgid, method, params]
    let request = Value::Array(vec![
        Value::from(0u64),         // type = request
        Value::from(msgid as u64), // msgid
        Value::from(method),       // method name
        Value::Array(args),        // params
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &request).context("Failed to encode msgpack request")?;
    stream
        .write_all(&buf)
        .context("Failed to write to neovim socket")?;
    stream.flush().context("Failed to flush neovim socket")?;

    // Read response
    let response = rmpv::decode::read_value(&mut stream)
        .context("Failed to read msgpack response from neovim")?;

    // Parse response: [1, msgid, error, result]
    let arr = response.as_array().context("Response is not an array")?;

    if arr.len() < 4 {
        anyhow::bail!("Response array too short: {:?}", arr);
    }

    // arr[0] should be 1 (response type)
    // arr[1] should be our msgid
    let error = &arr[2];
    let result = &arr[3];

    if !error.is_nil() {
        let err_msg = match error {
            Value::Array(parts) if parts.len() >= 2 => {
                format!("{}", parts[1])
            }
            Value::String(s) => s.as_str().unwrap_or("unknown error").to_string(),
            other => format!("{}", other),
        };
        anyhow::bail!("Neovim RPC error: {}", err_msg);
    }

    Ok(result.clone())
}

/// Execute a Vim ex-command (`:command`).
pub fn nvim_command(socket_path: &Path, cmd: &str) -> Result<()> {
    nvim_call(socket_path, "nvim_command", vec![Value::from(cmd)])?;
    Ok(())
}

/// Execute a Lua expression and return the result.
pub fn nvim_exec_lua(socket_path: &Path, code: &str, args: Vec<Value>) -> Result<Value> {
    nvim_call(
        socket_path,
        "nvim_exec_lua",
        vec![Value::from(code), Value::Array(args)],
    )
}

/// Extract an integer from a Value that may be a plain integer or
/// a Neovim Ext type (buffer/window/tabpage handles are encoded as Ext).
pub(crate) fn ext_or_int(val: &Value) -> i64 {
    match val {
        Value::Integer(n) => n.as_i64().unwrap_or(0),
        Value::Ext(_type_id, data) => {
            // Ext data is a msgpack-encoded integer; decode it
            if let Ok(v) = rmpv::decode::read_value(&mut &data[..]) {
                v.as_i64().unwrap_or(0)
            } else {
                // Fallback: interpret raw bytes as little-endian int
                let mut n: i64 = 0;
                for (i, &b) in data.iter().enumerate().take(8) {
                    n |= (b as i64) << (i * 8);
                }
                n
            }
        }
        _ => 0,
    }
}

/// Convert a msgpack Value to a readable string.
pub(crate) fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.as_str().unwrap_or("").to_string(),
        Value::Nil => String::new(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => format!("{}", i),
        Value::F32(f) => format!("{}", f),
        Value::F64(f) => format!("{}", f),
        other => format!("{}", other),
    }
}
