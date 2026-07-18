//! Success-path coverage for the editor LSP handlers via a mock Neovim
//! msgpack-RPC server.
//!
//! Each `nvim_call` opens a fresh Unix-socket connection and performs two
//! request/response exchanges: `nvim_get_mode` (confirm-prompt probe) then the
//! real method (`nvim_exec_lua`). Our mock binds the listener SYNCHRONOUSLY,
//! then a background thread answers a fixed number of connections. The
//! `nvim_exec_lua` reply is chosen by inspecting the Lua source so the handler's
//! post-RPC parse/serialize tail runs against realistic payloads.

use super::*;

use std::io::Write;
use std::os::unix::net::UnixListener;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use rmpv::Value;

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

async fn seed_socket(state: &ServerState, session_id: &str, sock: std::path::PathBuf) {
    let mut reg = state.nvim_registry.write().await;
    reg.insert((0, session_id.to_string()), sock);
}

async fn json_of(resp: axum::response::Response) -> (axum::http::StatusCode, serde_json::Value) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn lsp_query() -> EditorLspQuery {
    EditorLspQuery {
        path: "a.rs".into(),
        session_id: "sess".into(),
        line: Some(1),
        col: Some(0),
    }
}

/// Bind a Unix socket synchronously and answer `conns` connections in a
/// background thread, choosing each `nvim_exec_lua` result via `responder`.
/// Returns the socket path; the `TempDir` keeps the socket alive for the test.
fn spawn_mock_nvim<F>(conns: usize, responder: F) -> (tempfile::TempDir, std::path::PathBuf)
where
    F: Fn(&str, Option<&str>) -> Value + Send + Sync + 'static,
{
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("nvim.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    std::thread::spawn(move || {
        for _ in 0..conns {
            let mut stream = match listener.accept() {
                Ok((s, _)) => s,
                Err(_) => return,
            };
            // Answer every request on this connection until the client hangs up.
            loop {
                let req = match rmpv::decode::read_value(&mut stream) {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let arr = match req.as_array() {
                    Some(a) if a.len() >= 3 => a,
                    _ => break,
                };
                let msgid = arr[1].clone();
                let method = arr[2].as_str().unwrap_or("").to_string();
                let code = arr
                    .get(3)
                    .and_then(|p| p.as_array())
                    .and_then(|p| p.first())
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string());

                let result = if method == "nvim_get_mode" {
                    Value::Map(vec![
                        (Value::from("mode"), Value::from("n")),
                        (Value::from("blocking"), Value::from(false)),
                    ])
                } else {
                    responder(&method, code.as_deref())
                };

                let resp = Value::Array(vec![Value::from(1u64), msgid, Value::Nil, result]);
                let mut buf = Vec::new();
                rmpv::encode::write_value(&mut buf, &resp).unwrap();
                if stream.write_all(&buf).is_err() || stream.flush().is_err() {
                    break;
                }
            }
        }
    });

    (tmp, sock)
}

/// Standard `nvim_exec_lua` responder: buffer handle for `bufadd`, else the
/// caller-provided per-method payloads.
fn exec_reply(code: Option<&str>, diag: &str, hover: &str, def: &str, other: &str) -> Value {
    let code = code.unwrap_or("");
    if code.contains("bufadd") {
        Value::from(7i64)
    } else if code.contains("vim.diagnostic.get") {
        Value::from(diag.to_string())
    } else if code.contains("textDocument/hover") {
        Value::from(hover.to_string())
    } else if code.contains("textDocument/definition") {
        Value::from(def.to_string())
    } else {
        Value::from(other.to_string())
    }
}

// ── diagnostics ────────────────────────────────────────────────────────────

#[tokio::test]
async fn diagnostics_success_maps_json_array() {
    let diag = r#"[{"file":"a.rs","lnum":1,"col":1,"severity":"Error","message":"boom","source":"rustc"}]"#;
    let (_tmp_sock, sock) = spawn_mock_nvim(2, move |_m, c| exec_reply(c, diag, "", "", ""));
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_diagnostics(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["available"], true);
    assert_eq!(v["diagnostics"][0]["message"], "boom");
}

#[tokio::test]
async fn diagnostics_malformed_json_falls_back_to_empty() {
    let (_tmp_sock, sock) = spawn_mock_nvim(2, move |_m, c| {
        exec_reply(c, "this is not json", "", "", "")
    });
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_diagnostics(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    // Non-JSON reply → unwrap_or_else([]) branch.
    assert_eq!(v["diagnostics"], serde_json::json!([]));
}

// ── hover ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hover_success_non_json_is_some() {
    let (_tmp_sock, sock) =
        spawn_mock_nvim(2, move |_m, c| exec_reply(c, "", "fn foo() -> i32", "", ""));
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_hover(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    // Non-JSON hover text → Some(raw).
    assert_eq!(v["hover"], "fn foo() -> i32");
}

#[tokio::test]
async fn hover_error_json_is_none() {
    let (_tmp_sock, sock) = spawn_mock_nvim(2, move |_m, c| {
        exec_reply(
            c,
            "",
            r#"{"error":"No hover information available"}"#,
            "",
            "",
        )
    });
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_hover(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    // JSON payload containing "error" → None.
    assert_eq!(v["hover"], serde_json::Value::Null);
}

// ── definition ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn definition_success_extracts_locations() {
    let def = r#"{"locations":[{"file":"a.rs","lnum":3,"col":5}]}"#;
    let (_tmp_sock, sock) = spawn_mock_nvim(2, move |_m, c| exec_reply(c, "", "", def, ""));
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_definition(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["locations"][0]["lnum"], 3);
}

#[tokio::test]
async fn definition_no_locations_defaults_empty() {
    let (_tmp_sock, sock) = spawn_mock_nvim(2, move |_m, c| {
        exec_reply(c, "", "", r#"{"error":"No definition found"}"#, "")
    });
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_definition(State(state), auth(), Query(lsp_query()))
        .await
        .unwrap()
        .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["locations"], serde_json::json!([]));
}

// ── format ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn format_success_returns_file_content() {
    // find_or_load + format + write = 3 connections.
    let (_tmp_sock, sock) = spawn_mock_nvim(3, move |_m, c| {
        let code = c.unwrap_or("");
        if code.contains("bufadd") {
            Value::from(7i64)
        } else {
            // Both the format lua and the buffer-scoped write lua return a name string.
            Value::from("a.rs".to_string())
        }
    });
    let tmp = tempfile::TempDir::new().unwrap();
    // The handler reads the formatted file from disk afterwards.
    std::fs::write(tmp.path().join("a.rs"), "formatted contents\n").unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_format(
        State(state),
        auth(),
        axum::Json(EditorFormatRequest {
            path: "a.rs".into(),
            session_id: "sess".into(),
        }),
    )
    .await
    .unwrap()
    .into_response();
    let (st, v) = json_of(resp).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["formatted"], true);
    assert_eq!(v["content"], "formatted contents\n");
}

#[tokio::test]
async fn format_success_but_missing_file_500() {
    // Formatting succeeds over RPC but the file is absent on disk → read error.
    let (_tmp_sock, sock) = spawn_mock_nvim(3, move |_m, c| {
        let code = c.unwrap_or("");
        if code.contains("bufadd") {
            Value::from(7i64)
        } else {
            Value::from("x".to_string())
        }
    });
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", sock).await;

    let resp = editor_lsp_format(
        State(state),
        auth(),
        axum::Json(EditorFormatRequest {
            path: "missing.rs".into(),
            session_id: "sess".into(),
        }),
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}
