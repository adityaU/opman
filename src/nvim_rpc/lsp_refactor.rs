/// LSP refactoring operations: rename, format, signature help.
use std::path::Path;

use anyhow::Result;

use super::transport::{nvim_exec_lua, value_to_string};

/// Rename a symbol using LSP.
///
/// Renames the symbol at the given position (or cursor) to `new_name`.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_rename(
    socket_path: &Path,
    buf: i64,
    new_name: &str,
    line: Option<i64>,
    col: Option<i64>,
) -> Result<String> {
    let set_buf = if buf != 0 {
        format!("vim.api.nvim_set_current_buf({})", buf)
    } else {
        String::new()
    };
    let set_cursor = if let (Some(ln), Some(c)) = (line, col) {
        format!("vim.api.nvim_win_set_cursor(0, {{{}, {}}})", ln, c.max(0))
    } else {
        String::new()
    };

    let escaped_name = new_name.replace('\\', "\\\\").replace('"', "\\\"");

    let lua = format!(
        r#"
        {set_buf}
        {set_cursor}
        local new_name = "{name}"
        local params = vim.lsp.util.make_position_params(0)
        params.newName = new_name

        local results = vim.lsp.buf_request_sync(0, 'textDocument/rename', params, 10000)
        if not results then
            return vim.json.encode({{error = "No LSP response (timeout or no server)"}})
        end

        local changes = 0
        local files_changed = {{}}
        for client_id, res in pairs(results) do
            if res.result then
                -- Apply the workspace edit
                local client = vim.lsp.get_client_by_id(client_id)
                local enc = client and client.offset_encoding or "utf-16"
                vim.lsp.util.apply_workspace_edit(res.result, enc)

                -- Count changes
                if res.result.changes then
                    for uri, edits in pairs(res.result.changes) do
                        changes = changes + #edits
                        local file = vim.uri_to_fname(uri)
                        files_changed[file] = true
                    end
                end
                if res.result.documentChanges then
                    for _, dc in ipairs(res.result.documentChanges) do
                        if dc.edits then
                            changes = changes + #dc.edits
                            if dc.textDocument and dc.textDocument.uri then
                                local file = vim.uri_to_fname(dc.textDocument.uri)
                                files_changed[file] = true
                            end
                        end
                    end
                end
            end
        end

        if changes == 0 then
            return vim.json.encode({{error = "Rename produced no changes"}})
        end

        local file_list = {{}}
        for f, _ in pairs(files_changed) do
            table.insert(file_list, f)
        end
        table.sort(file_list)

        return vim.json.encode({{
            renamed_to = new_name,
            changes = changes,
            files = file_list,
            file_count = #file_list,
        }})
        "#,
        set_buf = set_buf,
        set_cursor = set_cursor,
        name = escaped_name,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Format a buffer using the LSP formatter.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_format(socket_path: &Path, buf: i64) -> Result<String> {
    let lua = format!(
        r#"
    local buf = {buf}
    if buf == 0 then buf = vim.api.nvim_get_current_buf() end
    local clients = vim.lsp.get_clients({{ bufnr = buf }})
    local has_formatter = false
    for _, client in ipairs(clients) do
        if client.supports_method("textDocument/formatting") then
            has_formatter = true
            break
        end
    end

    if not has_formatter then
        return vim.json.encode({{error = "No LSP server with formatting support attached to this buffer"}})
    end

    local buf_name = vim.api.nvim_buf_get_name(buf)
    vim.lsp.buf.format({{ async = false, timeout_ms = 10000, bufnr = buf }})

    return vim.json.encode({{
        formatted = true,
        file = buf_name,
    }})
    "#,
        buf = buf,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Get function signature help at the given position.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_signature(
    socket_path: &Path,
    buf: i64,
    line: Option<i64>,
    col: Option<i64>,
) -> Result<String> {
    let set_buf = if buf != 0 {
        format!("vim.api.nvim_set_current_buf({})", buf)
    } else {
        String::new()
    };
    let set_cursor = if let (Some(ln), Some(c)) = (line, col) {
        format!("vim.api.nvim_win_set_cursor(0, {{{}, {}}})", ln, c.max(0))
    } else {
        String::new()
    };

    let lua = format!(
        r#"
    {set_buf}
    {set_cursor}
    local params = vim.lsp.util.make_position_params(0)
    local results = vim.lsp.buf_request_sync(0, 'textDocument/signatureHelp', params, 5000)
    if not results then
        return vim.json.encode({{error = "No LSP response (timeout or no server)"}})
    end

    for _, res in pairs(results) do
        if res.result and res.result.signatures and #res.result.signatures > 0 then
            local sigs = res.result.signatures
            local active_sig = (res.result.activeSignature or 0) + 1
            local active_param = res.result.activeParameter or 0

            local out = {{}}
            for i, sig in ipairs(sigs) do
                local entry = {{
                    label = sig.label,
                    active = (i == active_sig),
                    documentation = nil,
                    parameters = {{}},
                    active_parameter = (i == active_sig) and active_param or nil,
                }}
                if sig.documentation then
                    if type(sig.documentation) == "string" then
                        entry.documentation = sig.documentation
                    elseif sig.documentation.value then
                        entry.documentation = sig.documentation.value
                    end
                end
                if sig.parameters then
                    for _, p in ipairs(sig.parameters) do
                        local param = {{ label = "" }}
                        if type(p.label) == "string" then
                            param.label = p.label
                        elseif type(p.label) == "table" then
                            -- [start, end] offsets into sig.label
                            param.label = sig.label:sub(p.label[1] + 1, p.label[2])
                        end
                        if p.documentation then
                            if type(p.documentation) == "string" then
                                param.documentation = p.documentation
                            elseif p.documentation.value then
                                param.documentation = p.documentation.value
                            end
                        end
                        table.insert(entry.parameters, param)
                    end
                end
                table.insert(out, entry)
            end

            return vim.json.encode({{signatures = out, count = #out}})
        end
    end

    return vim.json.encode({{error = "No signature help available"}})
    "#,
        set_buf = set_buf,
        set_cursor = set_cursor,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}
