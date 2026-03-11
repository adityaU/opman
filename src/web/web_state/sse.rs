// ── SSE stream consumer ─────────────────────────────────────────────

/// Connect to the opencode server's SSE `/event` endpoint and process
/// `message.updated` events to capture session stats.
/// Also re-broadcasts raw event data on `handle.raw_sse_tx` so the web
/// `session_events_stream` can forward them to browser clients without
/// opening a separate upstream connection.
///
/// Includes a heartbeat watchdog: if no data arrives within 60 seconds
/// (upstream opencode sends heartbeats every ~10s, axum keepalive every 15s),
/// the connection is considered stale and the function returns an error so the
/// caller can reconnect.
pub(super) async fn run_opencode_sse(
    handle: &super::WebStateHandle,
    base_url: &str,
    project_dir: &str,
) -> anyhow::Result<()> {
    use futures::StreamExt;

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        // No overall timeout — SSE streams are long-lived.
        // The heartbeat watchdog below handles stale connections.
        .build()?;

    let response = client
        .get(format!("{}/event", base_url))
        .header("Accept", "text/event-stream")
        .header("x-opencode-directory", project_dir)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("SSE endpoint returned status {}", response.status());
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // Heartbeat watchdog: if no bytes arrive within this duration, treat as stale.
    let stale_timeout = std::time::Duration::from_secs(60);

    loop {
        let chunk = tokio::time::timeout(stale_timeout, stream.next()).await;

        match chunk {
            Err(_elapsed) => {
                // Timed out waiting for data — connection is stale.
                anyhow::bail!(
                    "Upstream SSE stale for {}s (no data received)",
                    stale_timeout.as_secs()
                );
            }
            Ok(None) => {
                // Stream ended normally.
                break;
            }
            Ok(Some(Err(e))) => {
                anyhow::bail!("Upstream SSE read error: {}", e);
            }
            Ok(Some(Ok(chunk))) => {
                let text = String::from_utf8_lossy(&chunk);
                // Normalize CRLF → LF
                let text = text.replace("\r\n", "\n");
                buffer.push_str(&text);

                // Process complete SSE messages (separated by double newline)
                while let Some(boundary) = buffer.find("\n\n") {
                    let message: String = buffer.drain(..boundary).collect();
                    buffer.drain(..2); // consume the "\n\n" separator

                    if let Some(data) = extract_sse_data(&message) {
                        // Re-broadcast the raw event data to web clients
                        let _ = handle.raw_sse_tx.send(data.clone());

                        super::sse_handler::handle_web_sse_event(handle, &data, project_dir).await;
                    }
                }
            }
        }
    }

    Ok(())
}

pub(super) fn extract_sse_data(message: &str) -> Option<String> {
    let mut data_parts = Vec::new();
    for line in message.lines() {
        if let Some(stripped) = line.strip_prefix("data:") {
            data_parts.push(stripped.trim().to_string());
        }
    }
    if data_parts.is_empty() {
        None
    } else {
        Some(data_parts.join("\n"))
    }
}
