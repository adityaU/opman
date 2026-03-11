//! Terminal tool handlers (`web_terminal_*`).

use crate::web::types::{ServerState, WebEvent};

pub(crate) async fn handle_terminal_read(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;
    let last_n = args
        .get("last_n")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let output = state
        .pty_mgr
        .get_output(id)
        .await
        .ok_or_else(|| format!("PTY '{}' not found", id))?;

    let bytes = output.drain_new();
    if bytes.is_empty() {
        // If drain_new is empty, we've already read everything.
        // Return empty string to indicate no new output.
        return Ok("[no new output]".to_string());
    }

    let text = String::from_utf8_lossy(&bytes).to_string();
    match last_n {
        Some(n) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(n);
            Ok(lines[start..].join("\n"))
        }
        None => Ok(text),
    }
}

pub(crate) async fn handle_terminal_run(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'command' argument")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    // Write the command (with newline to execute)
    let mut data = command.as_bytes().to_vec();
    // Only append newline if it's not a control character (like Ctrl-C)
    if !command.starts_with('\x03') {
        data.push(b'\n');
    }

    let ok = state.pty_mgr.write(id, data).await;
    if !ok {
        return Err(format!("Failed to write to PTY '{}' (not found or dead)", id));
    }

    // Emit terminal focus event
    let _ = state.event_tx.send(WebEvent::McpTerminalFocus {
        id: id.to_string(),
    });

    if !wait {
        return Ok("Command sent".to_string());
    }

    // Wait for output to settle (poll until no new output for 500ms)
    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    let settle_duration = tokio::time::Duration::from_millis(500);
    let poll_interval = tokio::time::Duration::from_millis(100);
    let mut last_output_time = tokio::time::Instant::now();
    let mut accumulated = String::new();

    let output = state
        .pty_mgr
        .get_output(id)
        .await
        .ok_or_else(|| format!("PTY '{}' not found", id))?;

    // Small initial delay to let the command start producing output
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        let new_bytes = output.drain_new();
        if !new_bytes.is_empty() {
            accumulated.push_str(&String::from_utf8_lossy(&new_bytes));
            last_output_time = now;
        } else if now.duration_since(last_output_time) >= settle_duration {
            // Output has settled
            break;
        }

        tokio::time::sleep(poll_interval).await;
    }

    if accumulated.is_empty() {
        Ok("[command sent, no output captured]".to_string())
    } else {
        Ok(accumulated)
    }
}

pub(crate) async fn handle_terminal_list(state: &ServerState) -> Result<String, String> {
    let ids = state.pty_mgr.list().await;
    let result = serde_json::json!({
        "terminals": ids,
        "count": ids.len()
    });
    Ok(result.to_string())
}

pub(crate) async fn handle_terminal_new(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let rows = args
        .get("rows")
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u16;
    let cols = args
        .get("cols")
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as u16;

    // Generate a UUID for the new PTY
    let id = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand::random::<u32>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u64>() & 0xFFFFFFFFFFFF
    );

    // Get working directory from web state
    let working_dir = state
        .web_state
        .get_working_dir()
        .await
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    state
        .pty_mgr
        .spawn_shell(id.clone(), rows, cols, working_dir)
        .await
        .map_err(|e| format!("Failed to spawn PTY: {}", e))?;

    Ok(serde_json::json!({
        "id": id,
        "rows": rows,
        "cols": cols
    })
    .to_string())
}

pub(crate) async fn handle_terminal_close(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;

    let ok = state.pty_mgr.kill(id).await;
    if ok {
        Ok(format!("PTY '{}' closed", id))
    } else {
        Err(format!("PTY '{}' not found", id))
    }
}
