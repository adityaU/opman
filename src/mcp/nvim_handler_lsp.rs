use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};
use super::super::NvimOp;

pub(super) fn handle(
    socket: &Path,
    op: NvimOp,
    request: &SocketRequest,
    buf: i64,
) -> SocketResponse {
    match op {
        NvimOp::Diagnostics => match crate::nvim_rpc::nvim_lsp_diagnostics(
            socket,
            buf,
            request.buf_only.unwrap_or(false),
        ) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(error) => SocketResponse::err(format!("Failed to get diagnostics: {error}")),
        },
        NvimOp::Definition => call_position(
            socket,
            request,
            buf,
            |s, b, l, c| crate::nvim_rpc::nvim_lsp_definition(s, b, l, c),
            "Failed to get definition",
        ),
        NvimOp::References => call_position(
            socket,
            request,
            buf,
            |s, b, l, c| crate::nvim_rpc::nvim_lsp_references(s, b, l, c),
            "Failed to get references",
        ),
        NvimOp::Hover => call_position(
            socket,
            request,
            buf,
            |s, b, l, c| crate::nvim_rpc::nvim_lsp_hover(s, b, l, c),
            "Failed to get hover info",
        ),
        NvimOp::Symbols => match crate::nvim_rpc::nvim_lsp_symbols(
            socket,
            buf,
            request.query.as_deref().unwrap_or(""),
            request.workspace.unwrap_or(false),
        ) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(error) => SocketResponse::err(format!("Failed to get symbols: {error}")),
        },
        NvimOp::CodeActions => match crate::nvim_rpc::nvim_lsp_code_actions(socket, buf) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(error) => SocketResponse::err(format!("Failed to get code actions: {error}")),
        },
        NvimOp::Eval => {
            let Some(code) = request.command.as_deref() else {
                return SocketResponse::err("Missing 'command' (Lua code) for nvim_eval".into());
            };
            match crate::nvim_rpc::nvim_eval_lua(socket, code) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(error) => SocketResponse::err(format!("Lua eval failed: {error}")),
            }
        }
        NvimOp::Rename => {
            let Some(new_name) = request.new_name.as_deref() else {
                return SocketResponse::err("Missing 'new_name' for nvim_rename".into());
            };
            match crate::nvim_rpc::nvim_lsp_rename(socket, buf, new_name, request.line, request.col)
            {
                Ok(output) => SocketResponse::ok_text(output),
                Err(error) => SocketResponse::err(format!("Rename failed: {error}")),
            }
        }
        NvimOp::Format => match crate::nvim_rpc::nvim_lsp_format(socket, buf) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(error) => SocketResponse::err(format!("Format failed: {error}")),
        },
        NvimOp::Signature => call_position(
            socket,
            request,
            buf,
            |s, b, l, c| crate::nvim_rpc::nvim_lsp_signature(s, b, l, c),
            "Signature help failed",
        ),
        _ => SocketResponse::err(format!("Unsupported LSP operation: {op}")),
    }
}

fn call_position<F>(
    socket: &Path,
    request: &SocketRequest,
    buf: i64,
    call: F,
    label: &str,
) -> SocketResponse
where
    F: FnOnce(&Path, i64, Option<i64>, Option<i64>) -> anyhow::Result<String>,
{
    match call(socket, buf, request.line, request.col) {
        Ok(output) => SocketResponse::ok_text(output),
        Err(error) => SocketResponse::err(format!("{label}: {error}")),
    }
}
