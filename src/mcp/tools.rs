use std::path::Path;

use super::socket_client::{
    close_tab, format_mcp_response, poll_command_completion, send_socket_request,
};
use super::types::SocketRequest;

/// Handle a tools/call request by forwarding to the Unix socket.
pub async fn handle_tool_call(
    sock_path: &Path,
    params: Option<serde_json::Value>,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Handle ephemeral run as a composite operation (new → run → poll → close)
    if tool_name == "terminal_ephemeral_run" {
        return handle_ephemeral_run(sock_path, &arguments, session_id).await;
    }

    // Build internal socket request
    let socket_req = match tool_name {
        "terminal_read" => SocketRequest {
            op: "read".into(),
            tab: arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            last_n: arguments
                .get("last_n")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            ..Default::default()
        },
        "terminal_run" => {
            let cmd = arguments
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("terminal_run requires 'command' argument"))?;
            let wait = arguments.get("wait").and_then(|v| v.as_bool());
            SocketRequest {
                op: "run".into(),
                tab: arguments
                    .get("tab")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                command: Some(cmd.to_string()),
                wait,
                ..Default::default()
            }
        }
        "terminal_list" => SocketRequest {
            op: "list".into(),
            ..Default::default()
        },
        "terminal_new" => SocketRequest {
            op: "new".into(),
            name: arguments
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        },
        "terminal_close" => SocketRequest {
            op: "close".into(),
            tab: arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            ..Default::default()
        },
        "terminal_rename" => {
            let tab = arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| anyhow::anyhow!("terminal_rename requires 'tab' argument"))?;
            let name = arguments
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("terminal_rename requires 'name' argument"))?;
            SocketRequest {
                op: "rename".into(),
                tab: Some(tab),
                name: Some(name.to_string()),
                ..Default::default()
            }
        }
        other => {
            return Ok(serde_json::json!([{
                "type": "text",
                "text": format!("Unknown tool: {}", other)
            }]));
        }
    };

    // Inject session_id into the request for correct routing
    let mut socket_req = socket_req;
    socket_req.session_id = session_id.map(|s| s.to_string());

    // Check if this is a "run" with wait — we'll need the tab and wait flag before sending
    let is_wait_run = tool_name == "terminal_run" && socket_req.wait.unwrap_or(false);
    let wait_tab = socket_req.tab;
    let wait_timeout_secs = if is_wait_run {
        arguments
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
    } else {
        30
    };

    // Send the primary request
    let socket_resp = send_socket_request(sock_path, &socket_req).await?;

    // For "run" with wait=true: poll command_state until command finishes or timeout
    if is_wait_run && socket_resp.ok {
        let timed_out =
            poll_command_completion(sock_path, wait_tab, wait_timeout_secs, session_id).await;

        let read_req = SocketRequest {
            op: "read".into(),
            tab: wait_tab,
            session_id: session_id.map(|s| s.to_string()),
            ..Default::default()
        };
        let read_resp = send_socket_request(sock_path, &read_req).await?;

        if timed_out {
            let mut output = read_resp.output.unwrap_or_default();
            output = format!("[TIMEOUT after {}s]\n{}", wait_timeout_secs, output);
            return Ok(serde_json::json!([{
                "type": "text",
                "text": output
            }]));
        }

        return format_mcp_response(&read_resp);
    }

    format_mcp_response(&socket_resp)
}

async fn handle_ephemeral_run(
    sock_path: &Path,
    arguments: &serde_json::Value,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let cmd = arguments
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("terminal_ephemeral_run requires 'command' argument"))?;

    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("terminal_ephemeral_run requires 'name' argument"))?;

    let timeout_secs = arguments
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let sid = session_id.map(|s| s.to_string());

    // 0. Acquire ephemeral name lock — rejects if same name is already running
    let lock_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "ephemeral_lock".into(),
            name: Some(name.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await?;
    if !lock_resp.ok {
        let msg = lock_resp
            .error
            .unwrap_or_else(|| "Ephemeral lock failed".into());
        return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
    }

    // From here on, we must unlock on every exit path
    let result =
        handle_ephemeral_run_inner(sock_path, cmd, name, timeout_secs, sid.as_deref()).await;

    // Always release the ephemeral name lock
    let _ = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "ephemeral_unlock".into(),
            name: Some(name.to_string()),
            session_id: sid,
            ..Default::default()
        },
    )
    .await;

    result
}

async fn handle_ephemeral_run_inner(
    sock_path: &Path,
    cmd: &str,
    name: &str,
    timeout_secs: u64,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let sid = session_id.map(|s| s.to_string());

    // 1. Create ephemeral tab
    let new_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "new".into(),
            name: Some(name.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await?;

    let tab_idx = match new_resp.tab_index {
        Some(idx) => idx,
        None => {
            let msg = new_resp
                .error
                .unwrap_or_else(|| "Failed to create tab".into());
            return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
        }
    };

    // 2. Run the command in the ephemeral tab
    let run_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "run".into(),
            tab: Some(tab_idx),
            command: Some(cmd.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await;

    if let Err(e) = &run_resp {
        let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;
        return Err(anyhow::anyhow!("Failed to run command: {}", e));
    }
    if !run_resp.as_ref().unwrap().ok {
        let msg = run_resp
            .unwrap()
            .error
            .unwrap_or_else(|| "Run failed".into());
        let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;
        return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
    }

    let timed_out =
        poll_command_completion(sock_path, Some(tab_idx), timeout_secs, sid.as_deref()).await;

    let read_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "read".into(),
            tab: Some(tab_idx),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await;

    let mut final_output = match read_resp {
        Ok(ref r) if r.ok => r.output.clone().unwrap_or_default(),
        _ => String::new(),
    };

    if timed_out {
        final_output = format!("[TIMEOUT after {}s]\n{}", timeout_secs, final_output);
    }

    // 3. Close the ephemeral tab
    let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;

    Ok(serde_json::json!([{
        "type": "text",
        "text": final_output
    }]))
}
