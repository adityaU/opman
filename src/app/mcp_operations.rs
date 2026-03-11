use crate::mcp::SocketResponse;
use std::path::Path;

impl super::App {
    /// Handle all neovim-related MCP operations.
    ///
    /// Called from [`handle_mcp_request`](Self::handle_mcp_request) after the
    /// neovim socket address and buffer handle have been resolved.
    pub(crate) fn handle_nvim_operation(
        nvim_socket: &Path,
        buf: i64,
        request: &crate::mcp::SocketRequest,
    ) -> SocketResponse {
        match request.op.as_str() {
            "nvim_open" => {
                let file_path = match &request.file_path {
                    Some(p) => p.as_str(),
                    None => return SocketResponse::err("Missing 'file_path' for nvim_open".into()),
                };
                match crate::nvim_rpc::nvim_open_file(nvim_socket, file_path, request.line) {
                    Ok(()) => {
                        let mut msg = format!("Opened {}", file_path);
                        if let Some(ln) = request.line {
                            msg.push_str(&format!(" at line {}", ln));
                        }
                        SocketResponse::ok_text(msg)
                    }
                    Err(e) => SocketResponse::err(format!("Failed to open file in Neovim: {}", e)),
                }
            }
            "nvim_read" => {
                let start = request.line.unwrap_or(1).max(1) - 1;
                let end = match request.end_line {
                    Some(-1) | None => {
                        match crate::nvim_rpc::nvim_buf_line_count(nvim_socket, buf) {
                            Ok(count) => count,
                            Err(e) => {
                                return SocketResponse::err(format!(
                                    "Failed to get line count: {}",
                                    e
                                ))
                            }
                        }
                    }
                    Some(e) => e,
                };
                let lang = crate::nvim_rpc::nvim_buf_get_name(nvim_socket, buf)
                    .map(|name| crate::mcp_neovim::ext_to_lang(&name).to_string())
                    .unwrap_or_default();
                match crate::nvim_rpc::nvim_buf_get_lines(nvim_socket, buf, start, end) {
                    Ok(lines) => {
                        let numbered: Vec<String> = lines
                            .iter()
                            .enumerate()
                            .map(|(i, l)| format!("{}: {}", start + 1 + i as i64, l))
                            .collect();
                        let body = numbered.join("\n");
                        SocketResponse::ok_text(format!("```{}\n{}\n```", lang, body))
                    }
                    Err(e) => {
                        SocketResponse::err(format!("Failed to read lines from Neovim: {}", e))
                    }
                }
            }
            "nvim_command" => {
                let cmd = match &request.command {
                    Some(c) => c.as_str(),
                    None => {
                        return SocketResponse::err("Missing 'command' for nvim_command".into())
                    }
                };
                match crate::nvim_rpc::nvim_command(nvim_socket, cmd) {
                    Ok(()) => SocketResponse::ok_text(format!("Command executed: {}", cmd)),
                    Err(e) => SocketResponse::err(format!("Neovim command failed: {}", e)),
                }
            }
            "nvim_buffers" => match crate::nvim_rpc::nvim_list_bufs(nvim_socket) {
                Ok(bufs) => {
                    if bufs.is_empty() {
                        SocketResponse::ok_text("No named buffers loaded.".into())
                    } else {
                        let lines: Vec<String> = bufs
                            .iter()
                            .map(|(id, name)| format!("Buffer {}: {}", id, name))
                            .collect();
                        SocketResponse::ok_text(lines.join("\n"))
                    }
                }
                Err(e) => SocketResponse::err(format!("Failed to list buffers: {}", e)),
            },
            "nvim_info" => {
                let name = crate::nvim_rpc::nvim_buf_get_name(nvim_socket, buf)
                    .unwrap_or_else(|_| "(unknown)".into());
                let cursor = crate::nvim_rpc::nvim_cursor_pos(nvim_socket).unwrap_or((1, 0));
                let line_count =
                    crate::nvim_rpc::nvim_buf_line_count(nvim_socket, buf).unwrap_or(0);
                let info = format!(
                    "Buffer: {}\nCursor: line {}, column {}\nTotal lines: {}",
                    if name.is_empty() { "(unnamed)" } else { &name },
                    cursor.0,
                    cursor.1,
                    line_count
                );
                SocketResponse::ok_text(info)
            }
            "nvim_diagnostics" => match crate::nvim_rpc::nvim_lsp_diagnostics(
                nvim_socket,
                buf,
                request.buf_only.unwrap_or(false),
            ) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get diagnostics: {}", e)),
            },
            "nvim_definition" => match crate::nvim_rpc::nvim_lsp_definition(
                nvim_socket,
                buf,
                request.line,
                request.col,
            ) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get definition: {}", e)),
            },
            "nvim_references" => match crate::nvim_rpc::nvim_lsp_references(
                nvim_socket,
                buf,
                request.line,
                request.col,
            ) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get references: {}", e)),
            },
            "nvim_hover" => {
                match crate::nvim_rpc::nvim_lsp_hover(nvim_socket, buf, request.line, request.col) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Failed to get hover info: {}", e)),
                }
            }
            "nvim_symbols" => {
                let query = request.query.as_deref().unwrap_or("");
                let workspace = request.workspace.unwrap_or(false);
                match crate::nvim_rpc::nvim_lsp_symbols(nvim_socket, buf, query, workspace) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Failed to get symbols: {}", e)),
                }
            }
            "nvim_code_actions" => match crate::nvim_rpc::nvim_lsp_code_actions(nvim_socket, buf) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get code actions: {}", e)),
            },
            "nvim_eval" => {
                let code = match &request.command {
                    Some(c) => c.as_str(),
                    None => {
                        return SocketResponse::err(
                            "Missing 'command' (Lua code) for nvim_eval".into(),
                        )
                    }
                };
                match crate::nvim_rpc::nvim_eval_lua(nvim_socket, code) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Lua eval failed: {}", e)),
                }
            }
            "nvim_grep" => {
                let pattern = match &request.query {
                    Some(q) => q.as_str(),
                    None => {
                        return SocketResponse::err(
                            "Missing 'query' (search pattern) for nvim_grep".into(),
                        )
                    }
                };
                match crate::nvim_rpc::nvim_grep(nvim_socket, pattern, request.glob.as_deref()) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Grep failed: {}", e)),
                }
            }
            "nvim_diff" => match crate::nvim_rpc::nvim_buf_diff(nvim_socket, buf) {
                Ok(output) if output.is_empty() => {
                    SocketResponse::ok_text("No unsaved changes.".into())
                }
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to compute diff: {}", e)),
            },
            "nvim_write" => {
                match crate::nvim_rpc::nvim_write(nvim_socket, buf, request.all.unwrap_or(false)) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Failed to write: {}", e)),
                }
            }
            // ── Editing ──────────────────────────────────────
            "nvim_edit_and_save" => {
                if let Some(edit_ops) = &request.edits {
                    let mut resolved: Vec<crate::nvim_rpc::ResolvedEdit> = Vec::new();
                    for (i, op) in edit_ops.iter().enumerate() {
                        let edit_buf = match crate::nvim_rpc::nvim_find_or_load_buffer(
                            nvim_socket,
                            &op.file_path,
                        ) {
                            Ok(id) => id,
                            Err(e) => {
                                return SocketResponse::err(format!(
                                    "edits[{}]: failed to resolve buffer for '{}': {}",
                                    i, op.file_path, e
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
                    match crate::nvim_rpc::nvim_buf_multi_edit_and_save(nvim_socket, &mut resolved)
                    {
                        Ok(msg) => SocketResponse::ok_text(msg),
                        Err(e) => SocketResponse::err(format!("Multi-edit failed: {}", e)),
                    }
                } else {
                    let start_line = match request.line {
                        Some(l) => l,
                        None => {
                            return SocketResponse::err(
                                "Missing 'start_line' for nvim_edit_and_save".into(),
                            )
                        }
                    };
                    let end_line = match request.end_line {
                        Some(l) => l,
                        None => {
                            return SocketResponse::err(
                                "Missing 'end_line' for nvim_edit_and_save".into(),
                            )
                        }
                    };
                    let new_text = match &request.new_text {
                        Some(t) => t.as_str(),
                        None => {
                            return SocketResponse::err(
                                "Missing 'new_text' for nvim_edit_and_save".into(),
                            )
                        }
                    };
                    match crate::nvim_rpc::nvim_buf_set_text_and_save(
                        nvim_socket,
                        buf,
                        start_line,
                        end_line,
                        new_text,
                    ) {
                        Ok(msg) => SocketResponse::ok_text(msg),
                        Err(e) => SocketResponse::err(format!("Edit+save failed: {}", e)),
                    }
                }
            }
            "nvim_undo" => {
                let count = request.count.unwrap_or(1);
                match crate::nvim_rpc::nvim_undo(nvim_socket, buf, count) {
                    Ok(msg) => SocketResponse::ok_text(msg),
                    Err(e) => SocketResponse::err(format!("Undo failed: {}", e)),
                }
            }
            // ── LSP Refactoring ──────────────────────────────
            "nvim_rename" => {
                let new_name = match &request.new_name {
                    Some(n) => n.as_str(),
                    None => {
                        return SocketResponse::err("Missing 'new_name' for nvim_rename".into())
                    }
                };
                match crate::nvim_rpc::nvim_lsp_rename(
                    nvim_socket,
                    buf,
                    new_name,
                    request.line,
                    request.col,
                ) {
                    Ok(output) => SocketResponse::ok_text(output),
                    Err(e) => SocketResponse::err(format!("Rename failed: {}", e)),
                }
            }
            "nvim_format" => match crate::nvim_rpc::nvim_lsp_format(nvim_socket, buf) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Format failed: {}", e)),
            },
            "nvim_signature" => match crate::nvim_rpc::nvim_lsp_signature(
                nvim_socket,
                buf,
                request.line,
                request.col,
            ) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Signature help failed: {}", e)),
            },
            _ => unreachable!(),
        }
    }
}
