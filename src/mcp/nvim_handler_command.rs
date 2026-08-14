use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};

pub(super) fn handle(socket: &Path, request: &SocketRequest) -> SocketResponse {
    let Some(command) = request.command.as_deref() else {
        return SocketResponse::err("Missing 'command' for nvim_command".into());
    };
    match crate::nvim_rpc::nvim_command(socket, command) {
        Ok(()) => SocketResponse::ok_text(format!("Command executed: {command}")),
        Err(error) => SocketResponse::err(format!("Neovim command failed: {error}")),
    }
}
