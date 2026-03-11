use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::types::{SocketRequest, SocketResponse};

/// Send a SocketRequest over the Unix socket and return the response.
pub async fn send_socket_request(
    sock_path: &Path,
    request: &SocketRequest,
) -> anyhow::Result<SocketResponse> {
    let mut stream = tokio::net::UnixStream::connect(sock_path)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to connect to manager socket at {:?}: {}. Is opman running?",
                sock_path,
                e
            )
        })?;

    let req_json = serde_json::to_string(request)?;
    stream.write_all(req_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    // Shutdown write side so the server knows we're done sending
    stream.shutdown().await?;

    // Read response
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await?;
    let resp_str = String::from_utf8_lossy(&resp_buf);

    serde_json::from_str(resp_str.trim())
        .map_err(|e| anyhow::anyhow!("Invalid response from manager: {}", e))
}

/// Convert a SocketResponse to MCP content format.
pub fn format_mcp_response(socket_resp: &SocketResponse) -> anyhow::Result<serde_json::Value> {
    if !socket_resp.ok {
        let error_msg = socket_resp.error.as_deref().unwrap_or("Unknown error");
        return Ok(serde_json::json!([{
            "type": "text",
            "text": error_msg
        }]));
    }

    if let Some(ref output) = socket_resp.output {
        Ok(serde_json::json!([{
            "type": "text",
            "text": output
        }]))
    } else if let Some(ref tabs) = socket_resp.tabs {
        let tab_text = tabs
            .iter()
            .map(|t| {
                let name_part = if t.name.is_empty() {
                    String::new()
                } else {
                    format!(" \"{}\"", t.name)
                };
                let active_part = if t.active { " (active)" } else { "" };
                format!("Tab {}{}{}", t.index, name_part, active_part)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(serde_json::json!([{
            "type": "text",
            "text": tab_text
        }]))
    } else if let Some(tab_index) = socket_resp.tab_index {
        Ok(serde_json::json!([{
            "type": "text",
            "text": format!("Created new terminal tab at index {}", tab_index)
        }]))
    } else {
        Ok(serde_json::json!([{
            "type": "text",
            "text": "OK"
        }]))
    }
}

/// Poll command_state until the command finishes or timeout expires.
/// Returns true if timed out, false if command completed.
///
/// Two-phase polling:
/// 1. Wait for state to become "running" (shell started processing the command)
/// 2. Wait for state to leave "running" (command finished)
///
/// This avoids the race where we poll before the shell processes the command
/// and see a stale "idle"/"success"/"failure" from the previous command.
pub async fn poll_command_completion(
    sock_path: &Path,
    tab: Option<usize>,
    timeout_secs: u64,
    session_id: Option<&str>,
) -> bool {
    let poll_interval = std::time::Duration::from_millis(300);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    let status_req = SocketRequest {
        op: "status".into(),
        tab,
        session_id: session_id.map(|s| s.to_string()),
        ..Default::default()
    };

    // Phase 1: wait for state to become "running"
    loop {
        if std::time::Instant::now() >= deadline {
            return true;
        }

        tokio::time::sleep(poll_interval).await;

        let state = match send_socket_request(sock_path, &status_req).await {
            Ok(ref r) if r.ok => r.command_state.clone().unwrap_or_default(),
            _ => return false,
        };

        if state == "running" {
            break;
        }
    }

    // Phase 2: wait for state to leave "running"
    loop {
        if std::time::Instant::now() >= deadline {
            return true;
        }

        tokio::time::sleep(poll_interval).await;

        let state = match send_socket_request(sock_path, &status_req).await {
            Ok(ref r) if r.ok => r.command_state.clone().unwrap_or_default(),
            _ => return false,
        };

        match state.as_str() {
            "running" => continue,
            _ => return false,
        }
    }
}

pub async fn close_tab(
    sock_path: &Path,
    tab_idx: usize,
    session_id: Option<&str>,
) -> anyhow::Result<SocketResponse> {
    send_socket_request(
        sock_path,
        &SocketRequest {
            op: "close".into(),
            tab: Some(tab_idx),
            session_id: session_id.map(|s| s.to_string()),
            ..Default::default()
        },
    )
    .await
}
