use super::*;
use rmpv::Value;
use std::collections::HashMap;

#[test]
fn nvim_eval_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_exec_lua".into(), Value::from("42"));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_eval");
    req.command = Some("return 42".into());
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
    assert!(response.output.unwrap().contains("42"));
}

#[test]
fn lsp_ops_success() {
    for opname in [
        "nvim_diagnostics",
        "nvim_definition",
        "nvim_references",
        "nvim_hover",
        "nvim_symbols",
        "nvim_code_actions",
        "nvim_format",
        "nvim_signature",
    ] {
        let mut replies = HashMap::new();
        replies.insert("nvim_exec_lua".into(), Value::from("[]"));
        let mock = super::start_mock(replies);
        let response = super::invoke(&mock.path, &super::op(opname));
        assert!(
            response.ok,
            "{opname} should succeed, err: {:?}",
            response.error
        );
    }
}

#[test]
fn nvim_diagnostics_buf_only_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_exec_lua".into(), Value::from("[]"));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_diagnostics");
    req.buf_only = Some(true);
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
}

#[test]
fn nvim_symbols_workspace_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_exec_lua".into(), Value::from("[]"));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_symbols");
    req.query = Some("Foo".into());
    req.workspace = Some(true);
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
}

#[test]
fn nvim_rename_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_exec_lua".into(), Value::from("renamed"));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_rename");
    req.new_name = Some("Bar".into());
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
}

#[test]
fn nvim_edit_single_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_buf_get_name".into(), Value::from("/tmp/e.rs"));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_edit_and_save");
    req.line = Some(1);
    req.end_line = Some(2);
    req.new_text = Some("hello\nworld".into());
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
    let output = response.output.unwrap();
    assert!(output.contains("Replaced lines 1-2"));
    assert!(output.contains("Saved: /tmp/e.rs"));
}

#[test]
fn nvim_input_success() {
    let mut replies = HashMap::new();
    replies.insert("nvim_input".into(), Value::from(4i64));
    let mock = super::start_mock(replies);
    let mut req = super::op("nvim_input");
    req.input = Some("abcd".into());
    let response = super::invoke(&mock.path, &req);
    assert!(response.ok, "err: {:?}", response.error);
    assert!(response.output.unwrap().contains("Accepted 4 input bytes"));
}
