use std::path::Path;
use std::str::FromStr;

use super::types::{SocketRequest, SocketResponse};
use super::NvimOp;

#[path = "nvim_handler_command.rs"]
mod command;
#[path = "nvim_handler_edit.rs"]
mod edit;
#[path = "nvim_handler_input.rs"]
mod input;
#[path = "nvim_handler_lsp.rs"]
mod lsp;
#[path = "nvim_handler_open.rs"]
mod open;
#[path = "nvim_handler_read.rs"]
mod read;

pub fn handle_nvim_op_blocking(
    nvim_socket: &Path,
    op: NvimOp,
    request: &SocketRequest,
) -> SocketResponse {
    let buf = if op.needs_buffer() && request.edits.is_none() {
        resolve_buffer(nvim_socket, request)
    } else {
        Ok(0)
    };
    let buf = match buf {
        Ok(buf) => buf,
        Err(response) => return response,
    };

    match op {
        NvimOp::Open => open::handle(nvim_socket, request),
        NvimOp::Command => command::handle(nvim_socket, request),
        NvimOp::Input => input::handle(nvim_socket, request),
        NvimOp::Read => read::read(nvim_socket, request, buf),
        NvimOp::Buffers => read::buffers(nvim_socket),
        NvimOp::Info => read::info(nvim_socket, buf),
        NvimOp::Grep => read::grep(nvim_socket, request),
        NvimOp::Diff => read::diff(nvim_socket, buf),
        NvimOp::Write => edit::write(nvim_socket, request, buf),
        NvimOp::EditAndSave => edit::edit_and_save(nvim_socket, request, buf),
        NvimOp::Undo => edit::undo(nvim_socket, request, buf),
        NvimOp::Diagnostics
        | NvimOp::Definition
        | NvimOp::References
        | NvimOp::Hover
        | NvimOp::Symbols
        | NvimOp::CodeActions
        | NvimOp::Eval
        | NvimOp::Rename
        | NvimOp::Format
        | NvimOp::Signature => lsp::handle(nvim_socket, op, request, buf),
    }
}

fn resolve_buffer(nvim_socket: &Path, request: &SocketRequest) -> Result<i64, SocketResponse> {
    let Some(path) = request.file_path.as_deref() else {
        return Ok(0);
    };
    crate::nvim_rpc::nvim_find_or_load_buffer(nvim_socket, path).map_err(|error| {
        SocketResponse::err(format!("Failed to resolve buffer for '{path}': {error}"))
    })
}

pub(crate) fn handle_nvim_request(nvim_socket: &Path, request: &SocketRequest) -> SocketResponse {
    let op = match NvimOp::from_str(&request.op) {
        Ok(op) => op,
        Err(_) => return SocketResponse::err(format!("Unknown nvim operation: {}", request.op)),
    };
    handle_nvim_op_blocking(nvim_socket, op, request)
}

#[cfg(test)]
#[path = "nvim_handler_tests.rs"]
mod nvim_handler_tests;

#[cfg(test)]
#[path = "nvim_handler_success_tests.rs"]
mod nvim_handler_success_tests;
