/// LSP operations: diagnostics, definition, references, hover, symbols, code actions.
use std::path::Path;

use anyhow::Result;

use super::transport::{nvim_exec_lua, value_to_string};

/// Get LSP diagnostics for a specific buffer or all buffers.
///
/// Returns a JSON string with diagnostic entries.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_diagnostics(socket_path: &Path, buf: i64, buf_only: bool) -> Result<String> {
    let lua = format!(
        r#"
        local buf = {buf}
        if buf == 0 then buf = vim.api.nvim_get_current_buf() end
        if {buf_only} then
            local diags = vim.diagnostic.get(buf)
            local buf_name = vim.api.nvim_buf_get_name(buf)
            local out = {{}}
            for _, d in ipairs(diags) do
                table.insert(out, {{
                    file = buf_name,
                    lnum = d.lnum + 1,
                    col = d.col + 1,
                    severity = vim.diagnostic.severity[d.severity] or "Unknown",
                    message = d.message,
                    source = d.source or "",
                }})
            end
            return vim.json.encode(out)
        else
            local diags = vim.diagnostic.get()
            local out = {{}}
            for _, d in ipairs(diags) do
                local buf_name = vim.api.nvim_buf_get_name(d.bufnr or 0)
                table.insert(out, {{
                    file = buf_name,
                    lnum = d.lnum + 1,
                    col = d.col + 1,
                    severity = vim.diagnostic.severity[d.severity] or "Unknown",
                    message = d.message,
                    source = d.source or "",
                }})
            end
            return vim.json.encode(out)
        end
        "#,
        buf = buf,
        buf_only = if buf_only { "true" } else { "false" },
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Go to the definition of the symbol at the given position and return the location.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_definition(
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
    local results = vim.lsp.buf_request_sync(0, 'textDocument/definition', params, 5000)
    if not results then return vim.json.encode({{error = "No LSP response (timeout or no server)"}}) end

    local locations = {{}}
    for _, res in pairs(results) do
        if res.result then
            local items = res.result
            if items.uri or items.targetUri then items = {{items}} end
            for _, loc in ipairs(items) do
                local uri = loc.uri or loc.targetUri
                local range = loc.range or loc.targetSelectionRange or loc.targetRange
                if uri and range then
                    local file = vim.uri_to_fname(uri)
                    table.insert(locations, {{
                        file = file,
                        lnum = range.start.line + 1,
                        col = range.start.character + 1,
                    }})
                end
            end
        end
    end

    if #locations == 0 then
        return vim.json.encode({{error = "No definition found"}})
    end

    -- Jump to first result
    local first = locations[1]
    vim.cmd('edit ' .. vim.fn.fnameescape(first.file))
    vim.api.nvim_win_set_cursor(0, {{first.lnum, first.col - 1}})
    vim.cmd('normal! zz')

    return vim.json.encode({{locations = locations}})
    "#,
        set_buf = set_buf,
        set_cursor = set_cursor,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Find all references to the symbol at the given position.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_references(
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
    params.context = {{ includeDeclaration = true }}
    local results = vim.lsp.buf_request_sync(0, 'textDocument/references', params, 10000)
    if not results then return vim.json.encode({{error = "No LSP response (timeout or no server)"}}) end

    local refs = {{}}
    for _, res in pairs(results) do
        if res.result then
            for _, loc in ipairs(res.result) do
                local file = vim.uri_to_fname(loc.uri)
                local lnum = loc.range.start.line + 1
                local col = loc.range.start.character + 1
                local ok, lines = pcall(vim.fn.readfile, file)
                local text = ""
                if ok and lines and lines[lnum] then
                    text = vim.trim(lines[lnum])
                end
                table.insert(refs, {{
                    file = file,
                    lnum = lnum,
                    col = col,
                    text = text,
                }})
            end
        end
    end

    if #refs == 0 then
        return vim.json.encode({{error = "No references found"}})
    end

    return vim.json.encode({{references = refs, count = #refs}})
    "#,
        set_buf = set_buf,
        set_cursor = set_cursor,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Get hover/type information for the symbol at the given position.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_hover(
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
    local results = vim.lsp.buf_request_sync(0, 'textDocument/hover', params, 5000)
    if not results then return vim.json.encode({{error = "No LSP response (timeout or no server)"}}) end

    for _, res in pairs(results) do
        if res.result and res.result.contents then
            local contents = res.result.contents
            if type(contents) == "string" then
                return contents
            elseif contents.value then
                return contents.value
            elseif type(contents) == "table" then
                local parts = {{}}
                for _, c in ipairs(contents) do
                    if type(c) == "string" then
                        table.insert(parts, c)
                    elseif c.value then
                        table.insert(parts, c.value)
                    end
                end
                return table.concat(parts, "\n\n")
            end
        end
    end

    return vim.json.encode({{error = "No hover information available"}})
    "#,
        set_buf = set_buf,
        set_cursor = set_cursor,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// List available LSP code actions at the current cursor position.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_code_actions(socket_path: &Path, buf: i64) -> Result<String> {
    let set_buf = if buf != 0 {
        format!("vim.api.nvim_set_current_buf({})", buf)
    } else {
        String::new()
    };

    let lua = format!(
        r#"
    {set_buf}
    local params = vim.lsp.util.make_range_params(0)
    params.context = {{
        diagnostics = vim.diagnostic.get(0, {{ lnum = vim.api.nvim_win_get_cursor(0)[1] - 1 }}),
    }}
    local results = vim.lsp.buf_request_sync(0, 'textDocument/codeAction', params, 5000)
    if not results then return vim.json.encode({{error = "No LSP response"}}) end

    local actions = {{}}
    for _, res in pairs(results) do
        if res.result then
            for i, action in ipairs(res.result) do
                table.insert(actions, {{
                    index = i,
                    title = action.title,
                    kind = action.kind or "",
                    is_preferred = action.isPreferred or false,
                }})
            end
        end
    end

    if #actions == 0 then
        return vim.json.encode({{error = "No code actions available"}})
    end

    return vim.json.encode({{actions = actions, count = #actions}})
    "#,
        set_buf = set_buf,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}
