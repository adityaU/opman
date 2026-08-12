//! Wave-2 coverage for the SUCCESS arms of `handle_nvim_op_blocking`.
//!
//! Each op resolves through `crate::nvim_rpc`, which speaks msgpack-RPC over a
//! Unix socket. We stand up an in-process mock neovim that replies to every RPC
//! request with a canned result keyed by method name, so the ok-branches run
//! without a real neovim. The listener binds SYNCHRONOUSLY before the accept
//! thread starts (no connect race).
use super::*;
use crate::mcp::types::SocketRequest;
use rmpv::Value;
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

static SOCK_SEQ: AtomicU64 = AtomicU64::new(0);

fn invoke(socket: &Path, request: &SocketRequest) -> super::SocketResponse {
    let op: crate::mcp::NvimOp = request.op.parse().expect("known test operation");
    super::handle_nvim_op_blocking(socket, op, request)
}

/// A mock neovim msgpack-RPC server. Replies to `nvim_get_mode` with a normal
/// (non-`r?`) mode so `dismiss_confirm_prompts` returns immediately, and to
/// every other method with the caller-supplied canned result (default `Nil`).
struct MockNvim {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MockNvim {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

fn start_mock(replies: HashMap<String, Value>) -> MockNvim {
    let seq = SOCK_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "opman-mocknvim-{}-{}.sock",
        std::process::id(),
        seq
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let replies = Arc::new(replies);

    let handle = std::thread::spawn(move || {
        while !stop2.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).ok();
                    stream
                        .set_read_timeout(Some(Duration::from_millis(500)))
                        .ok();
                    loop {
                        let req = match rmpv::decode::read_value(&mut stream) {
                            Ok(v) => v,
                            Err(_) => break, // EOF / timeout → client done
                        };
                        let arr = match req.as_array() {
                            Some(a) if a.len() >= 4 => a,
                            _ => break,
                        };
                        let msgid = arr[1].as_u64().unwrap_or(0);
                        let method = arr[2].as_str().unwrap_or("").to_string();
                        let result = if method == "nvim_get_mode" {
                            Value::Map(vec![
                                (Value::from("mode"), Value::from("n")),
                                (Value::from("blocking"), Value::from(false)),
                            ])
                        } else {
                            replies.get(&method).cloned().unwrap_or(Value::Nil)
                        };
                        // Response: [1, msgid, error=nil, result]
                        let resp = Value::Array(vec![
                            Value::from(1u64),
                            Value::from(msgid),
                            Value::Nil,
                            result,
                        ]);
                        let mut buf = Vec::new();
                        if rmpv::encode::write_value(&mut buf, &resp).is_err() {
                            break;
                        }
                        if stream.write_all(&buf).is_err() {
                            break;
                        }
                        let _ = stream.flush();
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    MockNvim {
        path,
        stop,
        handle: Some(handle),
    }
}

fn op(name: &str) -> SocketRequest {
    SocketRequest {
        op: name.into(),
        ..Default::default()
    }
}

// ── success arms ─────────────────────────────────────────────────────────────

#[test]
fn nvim_command_success() {
    let mock = start_mock(HashMap::new());
    let mut req = op("nvim_command");
    req.command = Some("echo hi".into());
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("Command executed: echo hi"));
}

#[test]
fn nvim_open_success_no_line() {
    let mock = start_mock(HashMap::new());
    let mut req = op("nvim_open");
    req.file_path = Some("/tmp/whatever.rs".into());
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    let o = r.output.unwrap();
    assert!(o.contains("Opened /tmp/whatever.rs"));
    assert!(!o.contains("at line"));
}

#[test]
fn nvim_open_success_with_line() {
    let mock = start_mock(HashMap::new());
    let mut req = op("nvim_open");
    req.file_path = Some("/tmp/whatever.rs".into());
    req.line = Some(7);
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r
        .output
        .unwrap()
        .contains("Opened /tmp/whatever.rs at line 7"));
}

#[test]
fn nvim_info_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/x.rs"));
    replies.insert(
        "nvim_win_get_cursor".into(),
        Value::Array(vec![Value::from(3i64), Value::from(5i64)]),
    );
    replies.insert("nvim_buf_line_count".into(), Value::from(10i64));
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_info"));
    assert!(r.ok, "err: {:?}", r.error);
    let o = r.output.unwrap();
    assert!(o.contains("Buffer: /tmp/x.rs"));
    assert!(o.contains("Cursor: line 3, column 5"));
    assert!(o.contains("Total lines: 10"));
}

#[test]
fn nvim_buffers_success() {
    let mut replies = HashMap::new();
    replies.insert(
        "nvim_list_bufs".into(),
        Value::Array(vec![Value::from(1i64), Value::from(2i64)]),
    );
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/a.rs"));
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_buffers"));
    assert!(r.ok, "err: {:?}", r.error);
    let o = r.output.unwrap();
    assert!(o.contains("Buffer 1: /tmp/a.rs"));
    assert!(o.contains("Buffer 2: /tmp/a.rs"));
}

#[test]
fn nvim_buffers_empty_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_list_bufs".into(), Value::Array(vec![]));
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_buffers"));
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("No named buffers loaded"));
}

#[test]
fn nvim_read_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_buf_line_count".into(), Value::from(2i64));
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/f.rs"));
    replies.insert(
        "nvim_buf_get_lines".into(),
        Value::Array(vec![Value::from("alpha"), Value::from("beta")]),
    );
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_read"));
    assert!(r.ok, "err: {:?}", r.error);
    let o = r.output.unwrap();
    assert!(o.contains("1: alpha"));
    assert!(o.contains("2: beta"));
    assert!(o.contains("```rust"));
}

#[test]
fn nvim_read_explicit_range_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/f.txt"));
    replies.insert(
        "nvim_buf_get_lines".into(),
        Value::Array(vec![Value::from("only")]),
    );
    let mock = start_mock(replies);
    let mut req = op("nvim_read");
    req.line = Some(4);
    req.end_line = Some(5);
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    // start = 4 → line label begins at 4.
    assert!(r.output.unwrap().contains("4: only"));
}

#[test]
fn nvim_diff_success() {
    let mut replies = HashMap::new();
    replies.insert(
        "nvim_exec_lua".into(),
        Value::from("@@ -1 +1 @@\n-old\n+new"),
    );
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_diff"));
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("@@"));
}

#[test]
fn nvim_write_all_success() {
    let mock = start_mock(HashMap::new());
    let mut req = op("nvim_write");
    req.all = Some(true);
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("All buffers saved"));
}

#[test]
fn nvim_write_current_buffer_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/s.rs"));
    let mock = start_mock(replies);
    let r = invoke(&mock.path, &op("nvim_write"));
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("Saved: /tmp/s.rs"));
}

#[test]
fn nvim_undo_success() {
    let mock = start_mock(HashMap::new());
    let r = invoke(&mock.path, &op("nvim_undo"));
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("undo x1"));
}

#[test]
fn nvim_undo_redo_success() {
    let mock = start_mock(HashMap::new());
    let mut req = op("nvim_undo");
    req.count = Some(-2);
    let r = invoke(&mock.path, &req);
    assert!(r.ok, "err: {:?}", r.error);
    assert!(r.output.unwrap().contains("redo x2"));
}

#[path = "nvim_handler_success_extra_tests.rs"]
mod extra;
