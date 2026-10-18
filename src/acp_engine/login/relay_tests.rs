use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

#[test]
fn json_rpc_lines_are_frames_and_the_rest_is_text() {
    assert!(matches!(classify(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#), Line::Frame(_)));
    assert_eq!(classify("Open this link: https://x"), Line::Text("Open this link: https://x".into()));
    assert!(matches!(classify(r#"{"not":"rpc"}"#), Line::Text(_)));
    assert!(matches!(classify("{broken"), Line::Text(_)));
}

#[test]
fn local_addresses_are_spotted_in_links() {
    assert!(mentions_loopback("redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2F"));
    assert!(mentions_loopback("listening on http://localhost:9"));
    assert!(!mentions_loopback("https://accounts.google.com/o/oauth2"));
}

#[test]
fn only_local_addresses_are_relayed() {
    assert_eq!(redirect_target("  http://127.0.0.1:5/?code=1 \n"), Some("http://127.0.0.1:5/?code=1"));
    assert_eq!(redirect_target("http://localhost:5/"), Some("http://localhost:5/"));
    assert_eq!(redirect_target("https://evil.example/?code=1"), None);
    assert_eq!(redirect_target("http://127.0.0.1.evil.example/"), None);
    assert_eq!(redirect_target("http://localhost@evil.example/"), None);
}

#[tokio::test]
async fn a_redirect_reaches_the_local_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 1024];
        let read = socket.read(&mut buf).await.expect("read");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
            .await
            .expect("write");
        String::from_utf8_lossy(&buf[..read]).into_owned()
    });
    deliver(&format!("http://127.0.0.1:{port}/?code=abc")).await.expect("delivered");
    let request = server.await.expect("server");
    assert!(request.starts_with("GET /?code=abc "), "{request}");
}

#[tokio::test]
async fn nothing_listening_is_a_clear_error() {
    let error = deliver("http://127.0.0.1:1/").await.expect_err("refused");
    assert!(format!("{error:#}").contains("could not reach"));
}
