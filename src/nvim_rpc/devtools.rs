/// Dev-flow helpers: eval Lua, grep via vimgrep.
use std::path::Path;

use anyhow::Result;

use super::transport::{nvim_exec_lua, value_to_string};

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
