use std::path::Path;

use super::super::types::{SocketRequest, SocketResponse};

pub(super) fn read(socket: &Path, request: &SocketRequest, buf: i64) -> SocketResponse {
    let start = request.line.unwrap_or(1).max(1) - 1;
    let end = match request.end_line {
        Some(-1) | None => match crate::nvim_rpc::nvim_buf_line_count(socket, buf) {
            Ok(count) => count,
            Err(error) => return SocketResponse::err(format!("Failed to get line count: {error}")),
        },
        Some(end) => end,
    };
    let lang = crate::nvim_rpc::nvim_buf_get_name(socket, buf)
        .map(|name| crate::mcp_neovim::ext_to_lang(&name).to_string())
        .unwrap_or_default();
    match crate::nvim_rpc::nvim_buf_get_lines(socket, buf, start, end) {
        Ok(lines) => {
            let numbered: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(index, line)| format!("{}: {line}", start + 1 + index as i64))
                .collect();
            SocketResponse::ok_text(format!("```{lang}\n{}\n```", numbered.join("\n")))
        }
        Err(error) => SocketResponse::err(format!("Failed to read lines from Neovim: {error}")),
    }
}

pub(super) fn buffers(socket: &Path) -> SocketResponse {
    match crate::nvim_rpc::nvim_list_bufs(socket) {
        Ok(buffers) if buffers.is_empty() => {
            SocketResponse::ok_text("No named buffers loaded.".into())
        }
        Ok(buffers) => SocketResponse::ok_text(
            buffers
                .iter()
                .map(|(id, name)| format!("Buffer {id}: {name}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Err(error) => SocketResponse::err(format!("Failed to list buffers: {error}")),
    }
}

pub(super) fn info(socket: &Path, buf: i64) -> SocketResponse {
    let name =
        crate::nvim_rpc::nvim_buf_get_name(socket, buf).unwrap_or_else(|_| "(unknown)".into());
    let cursor = crate::nvim_rpc::nvim_cursor_pos(socket).unwrap_or((1, 0));
    let line_count = crate::nvim_rpc::nvim_buf_line_count(socket, buf).unwrap_or(0);
    SocketResponse::ok_text(format!(
        "Buffer: {}\nCursor: line {}, column {}\nTotal lines: {line_count}",
        if name.is_empty() { "(unnamed)" } else { &name },
        cursor.0,
        cursor.1,
    ))
}

pub(super) fn grep(socket: &Path, request: &SocketRequest) -> SocketResponse {
    let Some(pattern) = request.query.as_deref() else {
        return SocketResponse::err("Missing 'query' (search pattern) for nvim_grep".into());
    };
    match crate::nvim_rpc::nvim_grep(socket, pattern, request.glob.as_deref()) {
        Ok(output) => SocketResponse::ok_text(output),
        Err(error) => SocketResponse::err(format!("Grep failed: {error}")),
    }
}

pub(super) fn diff(socket: &Path, buf: i64) -> SocketResponse {
    match crate::nvim_rpc::nvim_buf_diff(socket, buf) {
        Ok(output) if output.is_empty() => SocketResponse::ok_text("No unsaved changes.".into()),
        Ok(output) => SocketResponse::ok_text(output),
        Err(error) => SocketResponse::err(format!("Failed to compute diff: {error}")),
    }
}
