use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};

pub(super) fn handle(socket: &Path, request: &SocketRequest) -> SocketResponse {
    let Some(input) = request.input.as_deref() else {
        return SocketResponse::err("Missing 'input' for nvim_input".into());
    };
    match crate::nvim_rpc::nvim_input(socket, input) {
        Ok(count) => SocketResponse::ok_text(format!("Accepted {} input bytes", count)),
        Err(error) => SocketResponse::err(format!("Neovim input failed: {}", error)),
    }
}
