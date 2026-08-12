use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};

pub(super) fn handle(socket: &Path, request: &SocketRequest) -> SocketResponse {
    let Some(path) = request.file_path.as_deref() else {
        return SocketResponse::err("Missing 'file_path' for nvim_open".into());
    };
    match crate::nvim_rpc::nvim_open_file(socket, path, request.line) {
        Ok(()) => {
            let suffix = request
                .line
                .map(|line| format!(" at line {}", line))
                .unwrap_or_default();
            SocketResponse::ok_text(format!("Opened {}{}", path, suffix))
        }
        Err(error) => SocketResponse::err(format!("Failed to open file in Neovim: {}", error)),
    }
}
