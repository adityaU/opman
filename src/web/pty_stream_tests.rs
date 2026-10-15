//! The terminal socket end to end: keystrokes in as binary frames, the PTY's echo out as
//! binary frames, against a real PTY running `cat` behind a real listener.

use std::time::{Duration, Instant};

use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::web::pty_manager::pty_test_support::{env_lock, write_fake_bin};
use crate::web::pty_manager::{start_web_pty_manager, PtyProgram, SpawnSpec};
use crate::web::test_support::test_server_state;

async fn serve_cat_pty(dir: &std::path::Path) -> std::net::SocketAddr {
    let sh = write_fake_bin(dir, "catshell", "exec cat");
    std::env::set_var("SHELL", sh.display().to_string());
    let mut state = test_server_state();
    state.pty_mgr = start_web_pty_manager();
    state
        .pty_mgr
        .spawn(SpawnSpec {
            id: "ws-term".into(),
            program: PtyProgram::Shell,
            project: dir.to_path_buf(),
            label: None,
            rows: 24,
            cols: 80,
        })
        .await
        .expect("cat spawns");
    let app = Router::new()
        .route("/pty/ws", get(terminal_socket))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

/// Read binary frames until `needle` shows up, or give up after `within`.
async fn read_until<S>(socket: &mut S, needle: &str, within: Duration) -> String
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let mut seen = String::new();
    let deadline = Instant::now() + within;
    while !seen.contains(needle) {
        let left = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(left, socket.next()).await {
            Ok(Some(Ok(Message::Binary(bytes)))) => seen.push_str(&String::from_utf8_lossy(&bytes)),
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }
    seen
}

#[tokio::test]
async fn keystrokes_go_in_and_the_echo_comes_back_on_one_socket() {
    let _g = env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let addr = serve_cat_pty(dir.path()).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/pty/ws?id=ws-term"))
        .await
        .expect("socket opens");

    socket
        .send(Message::Binary(b"hello opman\n".to_vec().into()))
        .await
        .expect("sends");
    let echoed = read_until(&mut socket, "hello opman", Duration::from_secs(3)).await;
    assert!(echoed.contains("hello opman"), "echo: {echoed:?}");

    // Output is pushed, not polled: a keystroke's echo no longer waits for a 50 ms tick.
    let started = Instant::now();
    socket.send(Message::Binary(b"z".to_vec().into())).await.expect("sends");
    let echoed = read_until(&mut socket, "z", Duration::from_secs(3)).await;
    assert!(echoed.contains('z'), "echo: {echoed:?}");
    assert!(started.elapsed() < Duration::from_millis(45), "echo took {:?}", started.elapsed());
}

#[tokio::test]
async fn an_unknown_pty_is_refused_before_the_upgrade() {
    let _g = env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let addr = serve_cat_pty(dir.path()).await;
    let refused = tokio_tungstenite::connect_async(format!("ws://{addr}/pty/ws?id=missing")).await;
    assert!(refused.is_err());
}
