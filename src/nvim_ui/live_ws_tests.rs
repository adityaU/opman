use std::process::Stdio;
use std::time::Duration;

use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::super::key::SessionKey;
use super::super::spawn::socket_path;
use super::helpers::{fixture, minimal_config};

#[tokio::test]
#[ignore = "spawns a real Neovim and WebSocket server"]
async fn websocket_closes_after_real_nvim_death() {
    if std::process::Command::new("nvim")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("skipping: nvim not installed");
        return;
    }
    let _live_guard = super::helpers::lock().await;
    let project = fixture();
    let state = crate::web::test_support::test_server_state_with_projects_and_nvim_config(
        vec![("u13-live-ws".into(), project.path().to_path_buf())],
        minimal_config(&project, "ws"),
    );
    let router = crate::web::test_support::test_router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let address = listener.local_addr().expect("test listener address");
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let key = SessionKey::new(0, "u13-ws-death");
    let url = format!(
        "ws://{address}/api/nvim/ui?project_idx=0&session_id={}",
        key.session_id
    );
    let (mut websocket, _) = connect_async(url).await.expect("WebSocket connects");
    loop {
        let message = tokio::time::timeout(Duration::from_secs(10), websocket.next())
            .await
            .expect("ready timeout")
            .expect("WebSocket closed before ready")
            .expect("WebSocket read");
        if let Message::Text(text) = message {
            if text.contains("\"ready\"") {
                break;
            }
        }
    }
    let socket = socket_path(&key);
    std::process::Command::new("pkill")
        .args([
            "-TERM",
            "-f",
            socket.to_str().expect("socket path is UTF-8"),
        ])
        .status()
        .expect("kill Neovim child");

    let mut saw_close = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = tokio::time::timeout(remaining, websocket.next())
            .await
            .expect("WebSocket did not close after Neovim death")
            .expect("WebSocket stream ended without a close frame")
            .expect("WebSocket read");
        match message {
            Message::Text(_) => {}
            Message::Close(_) => {
                saw_close = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_close, "dead Neovim WebSocket did not close");
    server.abort();
}
