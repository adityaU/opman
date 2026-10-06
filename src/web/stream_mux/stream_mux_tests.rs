//! The stream socket end to end: a real websocket, a test router with a fake SSE
//! endpoint behind it.

use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::{Extension, Json, Router};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::{stream_ws, RouterSlot};
use crate::web::auth::{create_jwt, AuthUser};
use crate::web::test_support::test_server_state_with_auth;

type Client = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Counts live fake streams; a stream's guard decrements it when its body is dropped.
#[derive(Clone, Default)]
struct Live(Arc<AtomicUsize>);

struct Guard(Live);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0 .0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Live {
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// `tag` events every 10ms with two-line data; needs auth like a real stream.
async fn fake_sse(
    _auth: AuthUser,
    Extension(live): Extension<Live>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    live.0.fetch_add(1, Ordering::SeqCst);
    let tag = q.get("tag").cloned().unwrap_or_default();
    let guard = Guard(live);
    let stream = futures::stream::unfold((0u32, guard), move |(n, guard)| {
        let tag = tag.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let event = Event::default().event(tag.clone()).data(format!("{tag}{n}\nsecond"));
            Some((Ok(event), (n + 1, guard)))
        }
    });
    Sse::new(stream)
}

async fn serve(live: Live) -> (std::net::SocketAddr, String) {
    let state = test_server_state_with_auth("user", "pass");
    let token = create_jwt("user", &state.jwt_secret).expect("jwt");
    let slot = RouterSlot::default();
    let router = Router::new()
        .route("/api/fake/sse", get(fake_sse))
        .route("/api/fake/json", get(|| async { Json(json!({"ok": true})) }))
        .route("/api/stream/ws", get(stream_ws))
        .layer(Extension(slot.clone()))
        .layer(Extension(live))
        .with_state(state);
    slot.fill(router.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (addr, token)
}

async fn dial(addr: std::net::SocketAddr, token: &str) -> Client {
    let mut request = format!("ws://{addr}/api/stream/ws")
        .into_client_request()
        .expect("request");
    let cookie = format!("opman_token={token}").parse().expect("header");
    request.headers_mut().insert("cookie", cookie);
    tokio_tungstenite::connect_async(request).await.expect("upgrade").0
}

async fn send(socket: &mut Client, frame: Value) {
    socket.send(Message::Text(frame.to_string().into())).await.expect("send");
}

/// The next frame that is not a keepalive.
async fn next(socket: &mut Client) -> Value {
    let read = async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let frame: Value = serde_json::from_str(text.as_str()).expect("json");
                    if frame["op"] != "ping" {
                        return frame;
                    }
                }
                Some(Ok(_)) => continue,
                other => panic!("socket ended: {other:?}"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), read).await.expect("a frame in time")
}

async fn wait_live(live: &Live, wanted: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while live.count() != wanted {
        assert!(tokio::time::Instant::now() < deadline, "live streams stayed at {}", live.count());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn concurrent_subscriptions_each_get_their_own_events() {
    let live = Live::default();
    let (addr, token) = serve(live.clone()).await;
    let mut socket = dial(addr, &token).await;
    send(&mut socket, json!({"op": "sub", "id": 1, "path": "/api/fake/sse?tag=a"})).await;
    send(&mut socket, json!({"op": "sub", "id": 2, "path": "/api/fake/sse?tag=b"})).await;

    let mut opened = Vec::new();
    let mut seen = [false, false];
    while !(seen[0] && seen[1]) || opened.len() < 2 {
        let frame = next(&mut socket).await;
        if frame["op"] == "open" {
            opened.push(frame["id"].as_u64());
            continue;
        }
        let id = frame["id"].as_u64().expect("id");
        let tag = if id == 1 { "a" } else { "b" };
        assert_eq!(frame["event"], tag);
        let data = frame["data"].as_str().expect("data");
        assert!(data.starts_with(tag) && data.ends_with("\nsecond"), "{data}");
        seen[(id - 1) as usize] = true;
    }
    assert_eq!(live.count(), 2);
}

#[tokio::test]
async fn unsubscribing_releases_the_stream_and_closing_releases_the_rest() {
    let live = Live::default();
    let (addr, token) = serve(live.clone()).await;
    let mut socket = dial(addr, &token).await;
    send(&mut socket, json!({"op": "sub", "id": 1, "path": "/api/fake/sse?tag=a"})).await;
    send(&mut socket, json!({"op": "sub", "id": 2, "path": "/api/fake/sse?tag=b"})).await;
    wait_live(&live, 2).await;

    send(&mut socket, json!({"op": "unsub", "id": 1})).await;
    wait_live(&live, 1).await;

    socket.close(None).await.expect("close");
    wait_live(&live, 0).await;
}

#[tokio::test]
async fn non_sse_and_unknown_paths_end_with_their_status() {
    let live = Live::default();
    let (addr, token) = serve(live.clone()).await;
    let mut socket = dial(addr, &token).await;
    let cases = [
        (1, "/api/fake/json", 415),
        (2, "/api/nope", 404),
        (3, "/internal/x", 400),
        (4, "/api/stream/ws", 400),
    ];
    for (id, path, _) in cases {
        send(&mut socket, json!({"op": "sub", "id": id, "path": path})).await;
    }
    let mut ends = std::collections::HashMap::new();
    while ends.len() < cases.len() {
        let frame = next(&mut socket).await;
        assert_eq!(frame["op"], "end", "{frame}");
        ends.insert(frame["id"].as_u64().expect("id"), frame["status"].as_u64());
    }
    for (id, _, status) in cases {
        assert_eq!(ends[&id], Some(status), "sub {id}");
    }
}

#[tokio::test]
async fn the_upgrade_requires_auth() {
    let (addr, _) = serve(Live::default()).await;
    let refused = tokio_tungstenite::connect_async(format!("ws://{addr}/api/stream/ws")).await;
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) = refused else {
        panic!("upgrade without credentials was accepted");
    };
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn a_query_token_authorizes_the_internal_requests() {
    let live = Live::default();
    let (addr, token) = serve(live.clone()).await;
    // A JWT may hold `+`, `/` and `=`, which a query string must escape.
    let token: String = url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
    let url = format!("ws://{addr}/api/stream/ws?token={token}");
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.expect("upgrade");
    send(&mut socket, json!({"op": "sub", "id": 9, "path": "/api/fake/sse?tag=q"})).await;
    assert_eq!(next(&mut socket).await, json!({"id": 9, "op": "open"}));
    assert_eq!(next(&mut socket).await["event"], "q");
}

#[test]
fn allowed_paths() {
    use super::subscription::allowed_uri;
    assert!(allowed_uri("/api/events").is_some());
    assert!(allowed_uri("/api/pty/stream?id=a%20b&replay=1").is_some());
    assert!(allowed_uri("/api/stream/ws/").is_none());
    assert!(allowed_uri("/remote/x/api/events").is_none());
    assert!(allowed_uri("http://evil/api/events").is_none());
}
