// ─── Tool dispatch: editing, dev-flow, LSP refactoring ───────────────────────

use crate::mcp::{EditOp, SocketRequest};

/// Dispatch editing, dev-flow, and LSP refactoring tools.
pub(super) fn dispatch_edit_devflow_refactor(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> anyhow::Result<SocketRequest> {
    match tool_name {
        "neovim_eval" => {
            let code = arguments
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_eval requires 'code' argument"))?;
            Ok(SocketRequest {
                op: "nvim_eval".into(),
                command: Some(code.to_string()),
                ..Default::default()
            })
        }
        "neovim_grep" => {
            let pattern = arguments
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_grep requires 'pattern' argument"))?;
            Ok(SocketRequest {
                op: "nvim_grep".into(),
                query: Some(pattern.to_string()),
                glob: arguments
                    .get("glob")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                ..Default::default()
            })
        }
        "neovim_edit_and_save" => build_edit_and_save_request(arguments),
        "neovim_undo" => {
            let file_path = arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow::anyhow!("neovim_undo requires 'file_path' argument — use neovim_buffers to find open buffers")
                })?;
            Ok(SocketRequest {
                op: "nvim_undo".into(),
                file_path: Some(file_path.to_string()),
                count: arguments.get("count").and_then(|v| v.as_i64()),
                ..Default::default()
            })
        }
        "neovim_rename" => {
            let new_name = arguments
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_rename requires 'new_name' argument"))?;
            Ok(SocketRequest {
                op: "nvim_rename".into(),
                file_path: arguments
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                new_name: Some(new_name.to_string()),
                line: arguments.get("line").and_then(|v| v.as_i64()),
                col: arguments.get("col").and_then(|v| v.as_i64()),
                ..Default::default()
            })
        }
        "neovim_format" => Ok(SocketRequest {
            op: "nvim_format".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        }),
        "neovim_signature" => Ok(SocketRequest {
            op: "nvim_signature".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        }),
        _ => unreachable!(),
    }
}

/// Parse the arguments for neovim_edit_and_save (single or batch).
fn build_edit_and_save_request(arguments: &serde_json::Value) -> anyhow::Result<SocketRequest> {
    // Check for multi-edit batch first
    if let Some(edits_val) = arguments.get("edits") {
        let edits_arr = edits_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("'edits' must be an array"))?;
        if edits_arr.is_empty() {
            anyhow::bail!("'edits' array must not be empty");
        }
        let mut edits = Vec::new();
        for (i, edit) in edits_arr.iter().enumerate() {
            let fp = edit
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("edits[{}] requires 'file_path'", i))?;
            let sl = edit
                .get("start_line")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("edits[{}] requires 'start_line'", i))?;
            let el = edit
                .get("end_line")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("edits[{}] requires 'end_line'", i))?;
            let nt = edit
                .get("new_text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("edits[{}] requires 'new_text'", i))?;
            edits.push(EditOp {
                file_path: fp.to_string(),
                start_line: sl,
                end_line: el,
                new_text: nt.to_string(),
            });
        }
        Ok(SocketRequest {
            op: "nvim_edit_and_save".into(),
            edits: Some(edits),
            ..Default::default()
        })
    } else {
        // Single edit — require all four params
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!("neovim_edit_and_save requires 'file_path' argument — use neovim_buffers to find open buffers")
            })?;
        let start_line = arguments
            .get("start_line")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                anyhow::anyhow!("neovim_edit_and_save requires 'start_line' argument")
            })?;
        let end_line = arguments
            .get("end_line")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("neovim_edit_and_save requires 'end_line' argument"))?;
        let new_text = arguments
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("neovim_edit_and_save requires 'new_text' argument"))?;
        Ok(SocketRequest {
            op: "nvim_edit_and_save".into(),
            file_path: Some(file_path.to_string()),
            line: Some(start_line),
            end_line: Some(end_line),
            new_text: Some(new_text.to_string()),
            ..Default::default()
        })
    }
}
