/// Buffer operations: find/load, read lines, list, open, diff, save, undo.
use std::path::Path;

use anyhow::{Context, Result};
use rmpv::Value;

use super::transport::{ext_or_int, nvim_call, nvim_command, nvim_exec_lua, value_to_string};

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

/// Undo or redo changes in a buffer.
///
/// Positive `count` undoes that many changes; negative `count` redoes.
/// Pass `buf = 0` for the current buffer.
pub fn nvim_undo(socket_path: &Path, buf: i64, count: i64) -> Result<String> {
    if buf != 0 {
        nvim_call(socket_path, "nvim_set_current_buf", vec![Value::from(buf)])?;
    }

    let (cmd, n) = if count >= 0 {
        ("undo", count.max(1))
    } else {
        ("redo", -count)
    };

    for _ in 0..n {
        nvim_command(socket_path, cmd)?;
    }

    Ok(format!("{} x{}", cmd, n))
}
