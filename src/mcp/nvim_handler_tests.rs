use super::*;
use crate::mcp::types::{EditOp, SocketRequest, SocketResponse};
use std::path::PathBuf;

/// A socket path that does not exist: every nvim_rpc call against it fails fast
/// with a connection error, exercising the error branches of each op.
fn bad_socket() -> PathBuf {
    let p = std::env::temp_dir().join(format!("opman-nvim-missing-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn run(req: SocketRequest) -> SocketResponse {
    let op = match req.op.parse() {
        Ok(op) => op,
        Err(_) => return SocketResponse::err(format!("Unknown nvim operation: {}", req.op)),
    };
    super::handle_nvim_op_blocking(&bad_socket(), op, &req)
}

fn op(name: &str) -> SocketRequest {
    SocketRequest {
        op: name.into(),
        ..Default::default()
    }
}

#[test]
fn unknown_op() {
    let r = run(op("nvim_bogus"));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("Unknown nvim operation"));
}

#[test]
fn nvim_open_missing_file_path() {
    let r = run(op("nvim_open"));
    assert!(r.error.unwrap().contains("Missing 'file_path'"));
}

#[test]
fn nvim_open_connect_error() {
    let mut req = op("nvim_open");
    req.file_path = Some("/tmp/whatever.rs".into());
    req.line = Some(10);
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to open file in Neovim"));
}

#[test]
fn buffer_resolution_failure() {
    // file_path set + op != nvim_open → resolve buffer, which fails.
    let mut req = op("nvim_read");
    req.file_path = Some("/tmp/some.rs".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to resolve buffer"));
}

#[test]
fn nvim_read_line_count_error_none() {
    let r = run(op("nvim_read"));
    assert!(r.error.unwrap().contains("Failed to get line count"));
}

#[test]
fn nvim_read_line_count_error_neg1() {
    let mut req = op("nvim_read");
    req.end_line = Some(-1);
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to get line count"));
}

#[test]
fn nvim_read_explicit_end_read_error() {
    let mut req = op("nvim_read");
    req.line = Some(2);
    req.end_line = Some(5);
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to read lines"));
}

#[test]
fn nvim_command_missing() {
    let r = run(op("nvim_command"));
    assert!(r
        .error
        .unwrap()
        .contains("Missing 'command' for nvim_command"));
}

#[test]
fn nvim_command_error() {
    let mut req = op("nvim_command");
    req.command = Some("echo hi".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Neovim command failed"));
}

#[test]
fn nvim_input_missing() {
    let r = run(op("nvim_input"));
    assert!(r.error.unwrap().contains("Missing 'input' for nvim_input"));
}

#[test]
fn nvim_input_connect_error() {
    let mut req = op("nvim_input");
    req.input = Some("<C-s>".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Neovim input failed"));
}

#[test]
fn nvim_buffers_error() {
    let r = run(op("nvim_buffers"));
    assert!(r.error.unwrap().contains("Failed to list buffers"));
}

#[test]
fn nvim_info_uses_defaults_on_error() {
    let r = run(op("nvim_info"));
    assert!(r.ok);
    let out = r.output.unwrap();
    assert!(out.contains("Buffer:"));
    assert!(out.contains("Cursor: line 1, column 0"));
    assert!(out.contains("Total lines: 0"));
}

#[test]
fn lsp_ops_error() {
    for (opname, needle) in [
        ("nvim_diagnostics", "Failed to get diagnostics"),
        ("nvim_definition", "Failed to get definition"),
        ("nvim_references", "Failed to get references"),
        ("nvim_hover", "Failed to get hover info"),
        ("nvim_symbols", "Failed to get symbols"),
        ("nvim_code_actions", "Failed to get code actions"),
        ("nvim_diff", "Failed to compute diff"),
        ("nvim_write", "Failed to write"),
        ("nvim_undo", "Undo failed"),
        ("nvim_format", "Format failed"),
        ("nvim_signature", "Signature help failed"),
    ] {
        let r = run(op(opname));
        assert!(!r.ok, "{opname} should fail");
        assert!(r.error.unwrap().contains(needle), "{opname} needle");
    }
}

#[test]
fn nvim_diagnostics_buf_only_flag() {
    let mut req = op("nvim_diagnostics");
    req.buf_only = Some(true);
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to get diagnostics"));
}

#[test]
fn nvim_symbols_with_query_and_workspace() {
    let mut req = op("nvim_symbols");
    req.query = Some("Foo".into());
    req.workspace = Some(true);
    let r = run(req);
    assert!(r.error.unwrap().contains("Failed to get symbols"));
}

#[test]
fn nvim_eval_missing() {
    let r = run(op("nvim_eval"));
    assert!(r.error.unwrap().contains("Missing 'command' (Lua code)"));
}

#[test]
fn nvim_eval_error() {
    let mut req = op("nvim_eval");
    req.command = Some("return 1".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Lua eval failed"));
}

#[test]
fn nvim_grep_missing_query() {
    let r = run(op("nvim_grep"));
    assert!(r.error.unwrap().contains("Missing 'query'"));
}

#[test]
fn nvim_grep_error_with_glob() {
    let mut req = op("nvim_grep");
    req.query = Some("TODO".into());
    req.glob = Some("*.rs".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Grep failed"));
}

#[test]
fn nvim_edit_missing_start_line() {
    let r = run(op("nvim_edit_and_save"));
    assert!(r.error.unwrap().contains("Missing 'start_line'"));
}

#[test]
fn nvim_edit_missing_end_line() {
    let mut req = op("nvim_edit_and_save");
    req.line = Some(1);
    let r = run(req);
    assert!(r.error.unwrap().contains("Missing 'end_line'"));
}

#[test]
fn nvim_edit_missing_new_text() {
    let mut req = op("nvim_edit_and_save");
    req.line = Some(1);
    req.end_line = Some(2);
    let r = run(req);
    assert!(r.error.unwrap().contains("Missing 'new_text'"));
}

#[test]
fn nvim_edit_single_error() {
    let mut req = op("nvim_edit_and_save");
    req.line = Some(1);
    req.end_line = Some(2);
    req.new_text = Some("hi".into());
    let r = run(req);
    assert!(r.error.unwrap().contains("Edit+save failed"));
}

#[test]
fn nvim_edit_batch_resolve_error() {
    let mut req = op("nvim_edit_and_save");
    req.edits = Some(vec![EditOp {
        file_path: "/tmp/a.rs".into(),
        start_line: 1,
        end_line: 2,
        new_text: "x".into(),
    }]);
    let r = run(req);
    assert!(r.error.unwrap().contains("edits[0]"));
}

#[test]
fn nvim_rename_missing_new_name() {
    let r = run(op("nvim_rename"));
    assert!(r.error.unwrap().contains("Missing 'new_name'"));
}

#[test]
fn nvim_rename_error() {
    let mut req = op("nvim_rename");
    req.new_name = Some("Bar".into());
    req.line = Some(3);
    req.col = Some(4);
    let r = run(req);
    assert!(r.error.unwrap().contains("Rename failed"));
}
