use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};

pub(super) fn write(socket: &Path, request: &SocketRequest, buf: i64) -> SocketResponse {
    match crate::nvim_rpc::nvim_write(socket, buf, request.all.unwrap_or(false)) {
        Ok(output) => SocketResponse::ok_text(output),
        Err(error) => SocketResponse::err(format!("Failed to write: {error}")),
    }
}

pub(super) fn edit_and_save(socket: &Path, request: &SocketRequest, buf: i64) -> SocketResponse {
    if let Some(edit_ops) = &request.edits {
        let mut resolved = Vec::with_capacity(edit_ops.len());
        for (index, op) in edit_ops.iter().enumerate() {
            let edit_buf = match crate::nvim_rpc::nvim_find_or_load_buffer(socket, &op.file_path) {
                Ok(id) => id,
                Err(error) => {
                    return SocketResponse::err(format!(
                        "edits[{index}]: failed to resolve buffer for '{}': {error}",
                        op.file_path
                    ))
                }
            };
            resolved.push(crate::nvim_rpc::ResolvedEdit {
                buf: edit_buf,
                file_path: op.file_path.clone(),
                start_line: op.start_line,
                end_line: op.end_line,
                new_text: op.new_text.clone(),
            });
        }
        return match crate::nvim_rpc::nvim_buf_multi_edit_and_save(socket, &mut resolved) {
            Ok(message) => SocketResponse::ok_text(message),
            Err(error) => SocketResponse::err(format!("Multi-edit failed: {error}")),
        };
    }

    let Some(start_line) = request.line else {
        return SocketResponse::err("Missing 'start_line' for nvim_edit_and_save".into());
    };
    let Some(end_line) = request.end_line else {
        return SocketResponse::err("Missing 'end_line' for nvim_edit_and_save".into());
    };
    let Some(new_text) = request.new_text.as_deref() else {
        return SocketResponse::err("Missing 'new_text' for nvim_edit_and_save".into());
    };
    match crate::nvim_rpc::nvim_buf_set_text_and_save(socket, buf, start_line, end_line, new_text) {
        Ok(message) => SocketResponse::ok_text(message),
        Err(error) => SocketResponse::err(format!("Edit+save failed: {error}")),
    }
}

pub(super) fn undo(socket: &Path, request: &SocketRequest, buf: i64) -> SocketResponse {
    match crate::nvim_rpc::nvim_undo(socket, buf, request.count.unwrap_or(1)) {
        Ok(message) => SocketResponse::ok_text(message),
        Err(error) => SocketResponse::err(format!("Undo failed: {error}")),
    }
}
