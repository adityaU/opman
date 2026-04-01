use std::path::{Path, PathBuf};

use tracing::info;

// ─── opencode.json auto-generation ──────────────────────────────────────────

/// Write (or update) the opencode.json file for a project to include the MCP server configs.
pub fn write_opencode_json(
    project_path: &Path,
    enable_terminal: bool,
    enable_neovim: bool,
    enable_time: bool,
    enable_ui: bool,
) -> anyhow::Result<()> {
    let json_path = project_path.join("opencode.json");

    // Read existing config or start fresh
    let mut config: serde_json::Value = if json_path.exists() {
        let content = std::fs::read_to_string(&json_path)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Get the current executable path for the MCP command
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman"));
    let exe_str = exe_path.to_string_lossy().to_string();

    let project_path_str = project_path.to_string_lossy().to_string();

    // Set mcp.* configs based on enabled flags
    let mcp = config
        .as_object_mut()
        .map(|obj| obj.entry("mcp").or_insert(serde_json::json!({})))
        .unwrap();

    if let Some(mcp_obj) = mcp.as_object_mut() {
        if enable_terminal {
            mcp_obj.insert(
                "terminal".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "mcp", &project_path_str]
                }),
            );
        } else {
            mcp_obj.remove("terminal");
        }
        if enable_neovim {
            mcp_obj.insert(
                "neovim".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "mcp-nvim", &project_path_str]
                }),
            );
        } else {
            mcp_obj.remove("neovim");
        }
        if enable_time {
            mcp_obj.insert(
                "time".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "mcp-time"]
                }),
            );
        } else {
            mcp_obj.remove("time");
        }
        if enable_ui {
            mcp_obj.insert(
                "ui".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "mcp-ui"]
                }),
            );
        } else {
            mcp_obj.remove("ui");
        }
    }

    // Disable opencode's native bash tool so it uses the manager's terminal instead
    if enable_terminal {
        let permission = config
            .as_object_mut()
            .map(|obj| obj.entry("permission").or_insert(serde_json::json!({})))
            .unwrap();
        if let Some(perm_obj) = permission.as_object_mut() {
            perm_obj.insert("bash".to_string(), serde_json::json!("deny"));
        }
    }

    // When neovim MCP is enabled, disable opencode's native edit/write tools
    // since the AI edits files through neovim directly.
    if enable_neovim {
        let permission = config
            .as_object_mut()
            .map(|obj| obj.entry("permission").or_insert(serde_json::json!({})))
            .unwrap();
        if let Some(perm_obj) = permission.as_object_mut() {
            perm_obj.insert("edit".to_string(), serde_json::json!("deny"));
        }
    }

    let formatted = serde_json::to_string_pretty(&config)?;
    std::fs::write(&json_path, formatted)?;
    info!(
        ?json_path,
        enable_terminal,
        enable_neovim,
        enable_time,
        enable_ui,
        "Wrote opencode.json with MCP config"
    );

    Ok(())
}
