// ─── Socket communication ────────────────────────────────────────────────────

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::mcp::{SocketRequest, SocketResponse};

/// Send a SocketRequest over the Unix socket and return the response.
pub(super) async fn send_socket_request(
    sock_path: &std::path::Path,
    request: &SocketRequest,
) -> anyhow::Result<SocketResponse> {
    let mut stream = UnixStream::connect(sock_path).await.map_err(|e| {
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

    // Shutdown write side so the server knows we are done sending
    stream.shutdown().await?;

    // Read response
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await?;
    let resp_str = String::from_utf8_lossy(&resp_buf);

    serde_json::from_str(resp_str.trim())
        .map_err(|e| anyhow::anyhow!("Invalid response from manager: {}", e))
}
