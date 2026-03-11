/// LSP symbol search: workspace and document symbols.
use std::path::Path;

use anyhow::Result;

use super::transport::{nvim_exec_lua, value_to_string};

/// Search for workspace or document symbols.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_lsp_symbols(
    socket_path: &Path,
    buf: i64,
    query: &str,
    workspace: bool,
) -> Result<String> {
    let method = if workspace {
        "workspace/symbol"
    } else {
        "textDocument/documentSymbol"
    };

    let escaped_query = query.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_method = method.replace('\\', "\\\\").replace('"', "\\\"");

    let lua = format!(
        r#"
        local buf = {buf}
        if buf == 0 then buf = vim.api.nvim_get_current_buf() end
        local method = "{method}"
        local params
        if method == "workspace/symbol" then
            params = {{ query = "{query}" }}
        else
            params = {{ textDocument = vim.lsp.util.make_text_document_params(buf) }}
        end

        local results = vim.lsp.buf_request_sync(buf, method, params, 10000)
        if not results then return vim.json.encode({{error = "No LSP response"}}) end

        local function flatten_symbols(symbols, file, prefix)
            local out = {{}}
            for _, s in ipairs(symbols) do
                local name = (prefix or "") .. s.name
                local kind = vim.lsp.protocol.SymbolKind[s.kind] or tostring(s.kind)
                local range = s.range or (s.location and s.location.range)
                local sym_file = file
                if s.location and s.location.uri then
                    sym_file = vim.uri_to_fname(s.location.uri)
                end
                local entry = {{
                    name = name,
                    kind = kind,
                    file = sym_file or "",
                    lnum = range and (range.start.line + 1) or 0,
                }}
                table.insert(out, entry)
                if s.children then
                    local children = flatten_symbols(s.children, sym_file, name .. ".")
                    for _, c in ipairs(children) do
                        table.insert(out, c)
                    end
                end
            end
            return out
        end

        local all = {{}}
        for _, res in pairs(results) do
            if res.result then
                local file = vim.api.nvim_buf_get_name(buf)
                local flat = flatten_symbols(res.result, file, nil)
                for _, s in ipairs(flat) do
                    table.insert(all, s)
                end
            end
        end

        if #all == 0 then
            return vim.json.encode({{error = "No symbols found"}})
        end

        return vim.json.encode({{symbols = all, count = #all}})
        "#,
        buf = buf,
        method = escaped_method,
        query = escaped_query,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}
