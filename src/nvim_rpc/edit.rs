/// Buffer editing operations: set text, set text and save, multi-edit batch.
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rmpv::Value;

use super::buffer::nvim_write;
use super::transport::{nvim_call, nvim_command, nvim_exec_lua};

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
    // Scroll to the edit target so the user sees the change happen.
    // nvim_win_set_cursor is 1-indexed (line, 0-indexed col).
    let _ = nvim_call(
        socket_path,
        "nvim_win_set_cursor",
        vec![
            Value::from(0i64), // current window
            Value::Array(vec![Value::from(start_line), Value::from(0i64)]),
        ],
    );
    let _ = nvim_command(socket_path, "normal! zz");

    let edit_msg = nvim_buf_set_text(socket_path, buf, start_line, end_line, new_text)?;
    let save_msg = nvim_write(socket_path, buf, false)?;

    // Highlight the edited range for 1 second.
    // start_line is 1-indexed; after the edit, new lines occupy
    // start_line .. start_line + new_lines_count - 1.
    let new_lines_count = if new_text.is_empty() {
        0i64
    } else {
        new_text.lines().count() as i64
    };
    if new_lines_count > 0 {
        // 0-indexed start/end for the highlight
        let hl_start = start_line - 1;
        let hl_end = hl_start + new_lines_count; // exclusive
        let lua = format!(
            r#"
            local buf = ({buf} == 0) and vim.api.nvim_get_current_buf() or {buf}
            local ns = vim.api.nvim_create_namespace("opman_edit_flash")
            vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
            for i = {start}, {end_exclusive} - 1 do
                vim.api.nvim_buf_add_highlight(buf, ns, "Visual", i, 0, -1)
            end
            vim.defer_fn(function()
                vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
            end, 1000)
            "#,
            buf = buf,
            start = hl_start,
            end_exclusive = hl_end,
        );
        // Best-effort: don't fail the edit if highlighting fails
        let _ = nvim_exec_lua(socket_path, &lua, vec![]);
    }

    Ok(format!("{}\n{}", edit_msg, save_msg))
}

/// A resolved edit ready to apply: buffer handle + line range + text.
pub struct ResolvedEdit {
    pub buf: i64,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub new_text: String,
}

/// Apply multiple edits across one or more files in a single batch.
///
/// Edits are applied sequentially in the order given. After each edit, the
/// line numbers of subsequent edits **to the same file** are adjusted by the
/// delta (lines_added − lines_removed) so callers can specify all line
/// numbers based on the **original** file contents.
///
/// Each modified file is saved once at the end. The Neovim viewport scrolls
/// to the first edit, and the last edited range in each file is flashed.
pub fn nvim_buf_multi_edit_and_save(
    socket_path: &Path,
    edits: &mut [ResolvedEdit],
) -> Result<String> {
    if edits.is_empty() {
        anyhow::bail!("No edits provided");
    }

    // Track cumulative line delta per file (keyed by file_path).
    let mut deltas: HashMap<String, i64> = HashMap::new();
    let mut messages: Vec<String> = Vec::new();
    // Track which buffers were modified (for saving) and last-edit info (for flash).
    let mut modified_bufs: HashMap<i64, String> = HashMap::new(); // buf -> file_path
    let mut last_edit_per_buf: HashMap<i64, (i64, i64)> = HashMap::new(); // buf -> (hl_start_0idx, hl_end_exclusive_0idx)

    for i in 0..edits.len() {
        let delta = *deltas.get(&edits[i].file_path).unwrap_or(&0);
        let adjusted_start = edits[i].start_line + delta;
        let adjusted_end = edits[i].end_line + delta;

        // Switch to the correct buffer and scroll to the edit location
        // so the user sees each edit happen.
        if edits[i].buf != 0 {
            let _ = nvim_call(
                socket_path,
                "nvim_set_current_buf",
                vec![Value::from(edits[i].buf)],
            );
        }
        let _ = nvim_call(
            socket_path,
            "nvim_win_set_cursor",
            vec![
                Value::from(0i64),
                Value::Array(vec![Value::from(adjusted_start), Value::from(0i64)]),
            ],
        );
        let _ = nvim_command(socket_path, "normal! zz");

        // Apply the edit
        let edit_msg = nvim_buf_set_text(
            socket_path,
            edits[i].buf,
            adjusted_start,
            adjusted_end,
            &edits[i].new_text,
        )?;
        messages.push(edit_msg);

        // Calculate the delta this edit introduced
        let lines_removed = edits[i].end_line - edits[i].start_line + 1;
        let lines_added = if edits[i].new_text.is_empty() {
            0i64
        } else {
            edits[i].new_text.lines().count() as i64
        };
        let edit_delta = lines_added - lines_removed;

        // Update cumulative delta for this file
        *deltas.entry(edits[i].file_path.clone()).or_insert(0) += edit_delta;

        // Track modified buffer and last edit position
        modified_bufs.insert(edits[i].buf, edits[i].file_path.clone());
        if lines_added > 0 {
            let hl_start = adjusted_start - 1; // 0-indexed
            let hl_end = hl_start + lines_added; // exclusive
            last_edit_per_buf.insert(edits[i].buf, (hl_start, hl_end));
        }
    }

    // Save each modified buffer once
    for (&buf, file_path) in &modified_bufs {
        match nvim_write(socket_path, buf, false) {
            Ok(msg) => messages.push(msg),
            Err(e) => messages.push(format!("Failed to save {}: {}", file_path, e)),
        }
    }

    // Flash the last edited range in each buffer for 1 second
    for (&buf, &(hl_start, hl_end)) in &last_edit_per_buf {
        let lua = format!(
            r#"
            local buf = ({buf} == 0) and vim.api.nvim_get_current_buf() or {buf}
            local ns = vim.api.nvim_create_namespace("opman_edit_flash")
            vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
            for i = {start}, {end_exclusive} - 1 do
                vim.api.nvim_buf_add_highlight(buf, ns, "Visual", i, 0, -1)
            end
            vim.defer_fn(function()
                vim.api.nvim_buf_clear_namespace(buf, ns, 0, -1)
            end, 1000)
            "#,
            buf = buf,
            start = hl_start,
            end_exclusive = hl_end,
        );
        let _ = nvim_exec_lua(socket_path, &lua, vec![]);
    }

    Ok(messages.join("\n"))
}
