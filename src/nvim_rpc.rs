/// Minimal Neovim MessagePack-RPC client.
///
/// Connects to a neovim `--listen` Unix socket and sends synchronous
/// requests using the msgpack-rpc protocol (type 0 = request, type 1 = response).
///
/// Protocol format (msgpack array):
///   Request:  [0, msgid, method, params]
///   Response: [1, msgid, error, result]
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use rmpv::Value;

/// Global message ID counter (monotonically increasing).
static MSG_ID: AtomicU32 = AtomicU32::new(1);

/// Check if neovim is in `r?` (confirm) mode and auto-dismiss the prompt.
///
/// When neovim encounters a swap file, file-changed-on-disk, or similar
/// situation it enters `r?` mode — a blocking confirmation dialog that
/// prevents any further RPC calls from completing. This function detects
/// that state and sends `E` (Edit anyway) to dismiss it, looping until
/// the mode clears.
fn dismiss_confirm_prompts(stream: &mut UnixStream) -> Result<()> {
    // Try up to 10 times in case prompts cascade (e.g. multiple swap files)
    for _ in 0..10 {
        let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let mode_request = Value::Array(vec![
            Value::from(0u64),
            Value::from(msgid as u64),
            Value::from("nvim_get_mode"),
            Value::Array(vec![]),
        ]);

        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &mode_request)
            .context("Failed to encode mode request")?;
        stream
            .write_all(&buf)
            .context("Failed to write mode request")?;
        stream.flush()?;

        let response =
            rmpv::decode::read_value(&mut *stream).context("Failed to read mode response")?;

        // Parse: [1, msgid, nil, {mode: "...", blocking: bool}]
        let is_confirm = response
            .as_array()
            .and_then(|arr| arr.get(3))
            .and_then(|result| result.as_map())
            .map(|pairs| {
                pairs
                    .iter()
                    .any(|(k, v)| k.as_str() == Some("mode") && v.as_str() == Some("r?"))
            })
            .unwrap_or(false);

        if !is_confirm {
            return Ok(());
        }

        // Dismiss the prompt by sending 'E' (Edit anyway)
        let input_msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let input_request = Value::Array(vec![
            Value::from(0u64),
            Value::from(input_msgid as u64),
            Value::from("nvim_input"),
            Value::Array(vec![Value::from("E")]),
        ]);

        let mut input_buf = Vec::new();
        rmpv::encode::write_value(&mut input_buf, &input_request)
            .context("Failed to encode input request")?;
        stream
            .write_all(&input_buf)
            .context("Failed to write input request")?;
        stream.flush()?;

        // Read and discard the nvim_input response
        let _ = rmpv::decode::read_value(&mut *stream);

        // Give neovim time to process before checking again
        std::thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Send an RPC request to neovim and return the result.
///
/// Connects fresh for each call (neovim's socket supports multiple connections).
/// Timeout prevents hanging if neovim is unresponsive.
/// Automatically dismisses any confirm-mode prompts before the actual call.
pub fn nvim_call(socket_path: &Path, method: &str, args: Vec<Value>) -> Result<Value> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("Failed to connect to neovim at {:?}", socket_path))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("Failed to set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("Failed to set write timeout")?;

    // Auto-dismiss any confirm prompts (swap file dialogs, etc.)
    dismiss_confirm_prompts(&mut stream)?;

    let msgid = MSG_ID.fetch_add(1, Ordering::Relaxed);

    // Encode request: [0, msgid, method, params]
    let request = Value::Array(vec![
        Value::from(0u64),         // type = request
        Value::from(msgid as u64), // msgid
        Value::from(method),       // method name
        Value::Array(args),        // params
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &request).context("Failed to encode msgpack request")?;
    stream
        .write_all(&buf)
        .context("Failed to write to neovim socket")?;
    stream.flush().context("Failed to flush neovim socket")?;

    // Read response
    let response = rmpv::decode::read_value(&mut stream)
        .context("Failed to read msgpack response from neovim")?;

    // Parse response: [1, msgid, error, result]
    let arr = response.as_array().context("Response is not an array")?;

    if arr.len() < 4 {
        anyhow::bail!("Response array too short: {:?}", arr);
    }

    // arr[0] should be 1 (response type)
    // arr[1] should be our msgid
    let error = &arr[2];
    let result = &arr[3];

    if !error.is_nil() {
        let err_msg = match error {
            Value::Array(parts) if parts.len() >= 2 => {
                format!("{}", parts[1])
            }
            Value::String(s) => s.as_str().unwrap_or("unknown error").to_string(),
            other => format!("{}", other),
        };
        anyhow::bail!("Neovim RPC error: {}", err_msg);
    }

    Ok(result.clone())
}

// ─── High-level helpers ─────────────────────────────────────────────────────

/// Execute a Vim ex-command (`:command`).
pub fn nvim_command(socket_path: &Path, cmd: &str) -> Result<()> {
    nvim_call(socket_path, "nvim_command", vec![Value::from(cmd)])?;
    Ok(())
}

/// Execute a Lua expression and return the result.
pub fn nvim_exec_lua(socket_path: &Path, code: &str, args: Vec<Value>) -> Result<Value> {
    nvim_call(
        socket_path,
        "nvim_exec_lua",
        vec![Value::from(code), Value::Array(args)],
    )
}

/// Find or load a buffer by file path and return its buffer handle.
///
/// Uses `vim.fn.bufadd()` (creates if needed) + `vim.fn.bufload()` (ensures
/// content is loaded). Returns the integer buffer handle suitable for passing
/// to `nvim_buf_get_lines`, `nvim_buf_set_lines`, etc.
pub fn nvim_find_or_load_buffer(socket_path: &Path, file_path: &str) -> Result<i64> {
    let escaped = file_path.replace('\\', "\\\\").replace('\'', "\\'");
    let lua = format!(
        r#"
        local buf = vim.fn.bufadd('{}')
        vim.fn.bufload(buf)
        return buf
        "#,
        escaped
    );
    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    result
        .as_i64()
        .context("nvim_find_or_load_buffer: expected integer buffer handle")
}

/// Get lines from a buffer (0-indexed, end-exclusive).
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_get_lines(
    socket_path: &Path,
    buf: i64,
    start: i64,
    end: i64,
) -> Result<Vec<String>> {
    let result = nvim_call(
        socket_path,
        "nvim_buf_get_lines",
        vec![
            Value::from(buf), // buffer handle (0 = current)
            Value::from(start),
            Value::from(end),
            Value::from(false), // strict_indexing
        ],
    )?;

    let lines = result
        .as_array()
        .context("Expected array of lines")?
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();

    Ok(lines)
}

/// Get the total number of lines in a buffer.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_line_count(socket_path: &Path, buf: i64) -> Result<i64> {
    let result = nvim_call(socket_path, "nvim_buf_line_count", vec![Value::from(buf)])?;
    result.as_i64().context("Expected integer line count")
}

/// Get the name (file path) of a buffer.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_get_name(socket_path: &Path, buf: i64) -> Result<String> {
    let result = nvim_call(socket_path, "nvim_buf_get_name", vec![Value::from(buf)])?;
    Ok(result.as_str().unwrap_or("").to_string())
}

/// Get the current cursor position as (line, col) — 1-indexed line, 0-indexed col.
pub fn nvim_cursor_pos(socket_path: &Path) -> Result<(i64, i64)> {
    let result = nvim_call(
        socket_path,
        "nvim_win_get_cursor",
        vec![Value::from(0i64)], // window 0 = current
    )?;
    let arr = result.as_array().context("Expected [line, col]")?;
    let line = arr.first().and_then(|v| v.as_i64()).unwrap_or(1);
    let col = arr.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
    Ok((line, col))
}

/// Extract an integer from a Value that may be a plain integer or
/// a Neovim Ext type (buffer/window/tabpage handles are encoded as Ext).
fn ext_or_int(val: &Value) -> i64 {
    match val {
        Value::Integer(n) => n.as_i64().unwrap_or(0),
        Value::Ext(_type_id, data) => {
            // Ext data is a msgpack-encoded integer; decode it
            if let Ok(v) = rmpv::decode::read_value(&mut &data[..]) {
                v.as_i64().unwrap_or(0)
            } else {
                // Fallback: interpret raw bytes as little-endian int
                let mut n: i64 = 0;
                for (i, &b) in data.iter().enumerate().take(8) {
                    n |= (b as i64) << (i * 8);
                }
                n
            }
        }
        _ => 0,
    }
}

/// List all loaded buffers with their names.
pub fn nvim_list_bufs(socket_path: &Path) -> Result<Vec<(i64, String)>> {
    let result = nvim_call(socket_path, "nvim_list_bufs", vec![])?;
    let bufs = result.as_array().context("Expected array of buffers")?;

    let mut out = Vec::new();
    for buf_id_val in bufs {
        let buf_id = ext_or_int(buf_id_val);
        // Get buffer name for each listed buffer
        let name_result = nvim_call(socket_path, "nvim_buf_get_name", vec![Value::from(buf_id)]);
        let name = name_result
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default();
        // Skip unnamed/empty buffers
        if !name.is_empty() {
            out.push((buf_id, name));
        }
    }
    Ok(out)
}

/// Open a file in neovim and optionally jump to a line.
pub fn nvim_open_file(socket_path: &Path, file_path: &str, line: Option<i64>) -> Result<()> {
    // Use fnameescape to safely handle special characters
    let escaped = file_path.replace('\'', "''");
    let cmd = format!("execute 'edit ' . fnameescape('{}')", escaped);
    nvim_command(socket_path, &cmd)?;

    if let Some(ln) = line {
        nvim_call(
            socket_path,
            "nvim_win_set_cursor",
            vec![
                Value::from(0i64), // current window
                Value::Array(vec![Value::from(ln), Value::from(0i64)]),
            ],
        )?;
        // Center the view
        nvim_command(socket_path, "normal! zz")?;
    }

    Ok(())
}

// ─── LSP helpers ────────────────────────────────────────────────────────────

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

// ─── Dev-flow helpers ───────────────────────────────────────────────────────

/// Execute arbitrary Lua code in Neovim and return the result as a string.
pub fn nvim_eval_lua(socket_path: &Path, code: &str) -> Result<String> {
    let result = nvim_exec_lua(socket_path, code, vec![])?;
    Ok(value_to_string(&result))
}

/// Search the project using Neovim's vimgrep and return matches.
pub fn nvim_grep(socket_path: &Path, pattern: &str, glob: Option<&str>) -> Result<String> {
    let escaped_pattern = pattern.replace('\\', "\\\\").replace('"', "\\\"");
    let file_glob = glob.unwrap_or("**/*");
    let escaped_glob = file_glob.replace('\\', "\\\\").replace('"', "\\\"");

    let lua = format!(
        r#"
        vim.fn.setqflist({{}}, 'r')
        local ok, err = pcall(vim.cmd, 'silent! vimgrep /{pattern}/gj {glob}')
        if not ok then
            if err and err:match("E480") then
                return vim.json.encode({{error = "No matches found"}})
            end
            return vim.json.encode({{error = tostring(err)}})
        end
        local qf = vim.fn.getqflist()
        local out = {{}}
        for _, item in ipairs(qf) do
            local file = ""
            if item.bufnr and item.bufnr > 0 then
                file = vim.api.nvim_buf_get_name(item.bufnr)
            end
            table.insert(out, {{
                file = file,
                lnum = item.lnum,
                col = item.col,
                text = vim.trim(item.text or ""),
            }})
        end
        return vim.json.encode({{matches = out, count = #out}})
        "#,
        pattern = escaped_pattern,
        glob = escaped_glob,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Get unsaved changes in a buffer as a unified diff.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_diff(socket_path: &Path, buf: i64) -> Result<String> {
    let lua = format!(
        r#"
    local buf = {buf}
    if buf == 0 then buf = vim.api.nvim_get_current_buf() end
    local name = vim.api.nvim_buf_get_name(buf)
    if name == "" then
        return vim.json.encode({{error = "Buffer has no file name"}})
    end

    local ok, on_disk = pcall(vim.fn.readfile, name)
    if not ok then
        return vim.json.encode({{error = "File not on disk (new file)"}})
    end

    local current = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
    local modified = vim.bo[buf].modified
    if not modified then
        return "No unsaved changes."
    end

    if vim.diff then
        local a = table.concat(on_disk, "\n") .. "\n"
        local b = table.concat(current, "\n") .. "\n"
        local diff = vim.diff(a, b, {{result_type = "unified", ctxlen = 3}})
        if diff == "" then
            return "No differences."
        end
        return diff
    end

    return vim.json.encode({{error = "vim.diff not available (requires Neovim 0.9+)"}})
    "#,
        buf = buf,
    );

    let result = nvim_exec_lua(socket_path, &lua, vec![])?;
    Ok(value_to_string(&result))
}

/// Save a buffer or all buffers.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_write(socket_path: &Path, buf: i64, all: bool) -> Result<String> {
    if all {
        nvim_command(socket_path, "wall")?;
        Ok("All buffers saved.".to_string())
    } else if buf == 0 {
        nvim_command(socket_path, "write")?;
        let name = nvim_buf_get_name(socket_path, 0).unwrap_or_default();
        Ok(format!(
            "Saved: {}",
            if name.is_empty() { "(unnamed)" } else { &name }
        ))
    } else {
        // Scope to specific buffer
        let lua = format!(
            r#"
            vim.api.nvim_buf_call({buf}, function()
                vim.cmd('write')
            end)
            return vim.api.nvim_buf_get_name({buf})
            "#,
            buf = buf,
        );
        let result = nvim_exec_lua(socket_path, &lua, vec![])?;
        let name = value_to_string(&result);
        Ok(format!(
            "Saved: {}",
            if name.is_empty() { "(unnamed)" } else { &name }
        ))
    }
}

/// Replace lines in a buffer.
///
/// `start_line` and `end_line` are 1-indexed, inclusive.
/// The new text replaces the specified range. Pass an empty string to delete lines.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_set_text(
    socket_path: &Path,
    buf: i64,
    start_line: i64,
    end_line: i64,
    new_text: &str,
) -> Result<String> {
    // Convert from 1-indexed inclusive to 0-indexed start, exclusive end
    let start = start_line - 1;
    let end = end_line; // end is already exclusive in nvim API when 1-indexed inclusive

    // Split new_text into lines for nvim_buf_set_lines
    let replacement: Vec<Value> = if new_text.is_empty() {
        vec![]
    } else {
        new_text.lines().map(|l| Value::from(l)).collect()
    };

    nvim_call(
        socket_path,
        "nvim_buf_set_lines",
        vec![
            Value::from(buf),          // buffer handle (0 = current)
            Value::from(start),        // start (0-indexed)
            Value::from(end),          // end (exclusive)
            Value::from(false),        // strict_indexing
            Value::Array(replacement), // replacement lines
        ],
    )?;

    let lines_removed = end_line - start_line + 1;
    let lines_added = if new_text.is_empty() {
        0
    } else {
        new_text.lines().count() as i64
    };

    Ok(format!(
        "Replaced lines {}-{} ({} lines removed, {} lines added)",
        start_line, end_line, lines_removed, lines_added
    ))
}

/// Replace lines in a buffer and save to disk.
///
/// Combines `nvim_buf_set_text` + `nvim_write` in a single operation.
/// `start_line` and `end_line` are 1-indexed, inclusive.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_buf_set_text_and_save(
    socket_path: &Path,
    buf: i64,
    start_line: i64,
    end_line: i64,
    new_text: &str,
) -> Result<String> {
    let edit_msg = nvim_buf_set_text(socket_path, buf, start_line, end_line, new_text)?;
    let save_msg = nvim_write(socket_path, buf, false)?;
    Ok(format!("{}\n{}", edit_msg, save_msg))
}

/// Undo or redo changes in a buffer.
///
/// `count` is the number of times to undo (positive) or redo (negative).
/// Defaults to 1 undo if count is 0 or positive, redo if negative.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_undo(socket_path: &Path, buf: i64, count: i64) -> Result<String> {
    let (cmd, n) = if count < 0 {
        ("redo", (-count) as u64)
    } else {
        ("undo", count.max(1) as u64)
    };

    if buf == 0 {
        // Current buffer: use simple commands
        for _ in 0..n {
            nvim_command(socket_path, cmd)?;
        }
    } else {
        // Specific buffer: scope via nvim_buf_call
        let lua = format!(
            r#"
            vim.api.nvim_buf_call({buf}, function()
                for _ = 1, {n} do
                    vim.cmd('{cmd}')
                end
            end)
            "#,
            buf = buf,
            n = n,
            cmd = cmd,
        );
        nvim_exec_lua(socket_path, &lua, vec![])?;
    }

    Ok(format!("{} x{}", cmd, n))
}

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

// ─── Utility ────────────────────────────────────────────────────────────────

/// Convert a msgpack Value to a readable string.
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.as_str().unwrap_or("").to_string(),
        Value::Nil => String::new(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => format!("{}", i),
        Value::F32(f) => format!("{}", f),
        Value::F64(f) => format!("{}", f),
        other => format!("{}", other),
    }
}
