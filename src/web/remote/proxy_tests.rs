use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Bytes, Message};

use super::fake_remote::setup;

#[tokio::test]
async fn proxies_json_with_method_query_body_and_filters_headers() {
    let s = setup().await;
    let response = reqwest::Client::new()
        .post(format!("{}/remote/{}/api/echo/a/b?x=1&token=home", s.home, s.id))
        .header("x-test", "kept")
        .header("cookie", "opman_token=home")
        .body("payload")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("set-cookie").is_none());
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["method"], "POST");
    assert_eq!(body["path"], "/api/echo/a/b");
    assert_eq!(body["query"], "x=1");
    assert_eq!(body["cookie"], false);
    assert_eq!(body["x_test"], "kept");
    assert_eq!(body["body"], "payload");
}

#[tokio::test]
async fn sse_first_event_arrives_before_upstream_finishes() {
    let s = setup().await;
    let response = reqwest::get(format!("{}/remote/{}/api/sse", s.home, s.id))
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut stream = response.bytes_stream();
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("first event was buffered")
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("one"));
    s.fake.release.notify_one();
    let mut rest = Vec::new();
    while let Some(chunk) = stream.next().await {
        rest.extend_from_slice(&chunk.unwrap());
    }
    assert!(String::from_utf8_lossy(&rest).contains("two"));
}

#[tokio::test]
async fn replayable_request_relogs_in_after_401() {
    let s = setup().await;
    let logins = s.fake.logins();
    s.fake.rotate();
    let response = reqwest::Client::new()
        .put(format!("{}/remote/{}/api/echo/x", s.home, s.id))
        .body("again")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(s.fake.logins(), logins + 1);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["body"], "again");
}

#[tokio::test]
async fn streamed_request_body_returns_the_401() {
    let s = setup().await;
    s.fake.rotate();
    let chunks = futures::stream::iter([Ok::<_, std::io::Error>("a"), Ok("b")]);
    let response = reqwest::Client::new()
        .post(format!("{}/remote/{}/api/echo/x", s.home, s.id))
        .body(reqwest::Body::wrap_stream(chunks))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn unknown_or_malformed_server_is_404() {
    let s = setup().await;
    let missing = reqwest::get(format!("{}/remote/ghost/api/echo/x", s.home))
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    let bad_id = reqwest::get(format!("{}/remote/BAD!/api/echo/x", s.home))
        .await
        .unwrap();
    assert_eq!(bad_id.status(), 404);
}

#[tokio::test]
async fn websocket_echoes_through_the_proxy_and_relogs_in() {
    let s = setup().await;
    s.fake.rotate();
    let url = format!(
        "{}/remote/{}/api/ws?token=home",
        s.home.replace("http://", "ws://"),
        s.id
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    socket.send(Message::text("hello")).await.unwrap();
    assert_eq!(socket.next().await.unwrap().unwrap(), Message::text("hello"));
    let bytes = Bytes::from_static(&[0, 1, 2, 255]);
    socket.send(Message::Binary(bytes.clone())).await.unwrap();
    assert_eq!(socket.next().await.unwrap().unwrap(), Message::Binary(bytes));
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn registry_connect_ws_is_authenticated() {
    let s = setup().await;
    let mut socket = s.registry.connect_ws(&s.id, "/api/ws").await.unwrap();
    socket.send(Message::text("direct")).await.unwrap();
    assert_eq!(socket.next().await.unwrap().unwrap(), Message::text("direct"));
}
