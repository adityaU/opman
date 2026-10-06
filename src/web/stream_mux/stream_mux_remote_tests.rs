//! The stream socket of a remote server, reached through home's `/remote/{id}/api/*`
//! WebSocket proxy: the remote authorizes its internal requests with the bearer token
//! home logged in with.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::remote::{NewServer, Registry, ServerStore};
use crate::web::test_support::{
    start_mock_upstream, test_router, test_server_state, test_server_state_with_auth,
};

#[tokio::test]
async fn a_remote_stream_socket_works_through_the_home_proxy() {
    let remote = start_mock_upstream(test_router(test_server_state_with_auth("ru", "rp"))).await;
    let registry = Registry::new(ServerStore::memory());
    let added = registry
        .add(NewServer {
            name: "Remote".into(),
            url: remote,
            username: Some("ru".into()),
            password: Some("rp".into()),
        })
        .await
        .expect("add remote");
    let mut home_state = test_server_state();
    home_state.remote = registry;
    let home = start_mock_upstream(test_router(home_state)).await;

    let url = format!(
        "{}/remote/{}/api/stream/ws",
        home.replace("http://", "ws://"),
        added.id
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.expect("upgrade");
    let subs = [(1, "/api/events"), (2, "/api/auth/verify")];
    for (id, path) in subs {
        let frame = json!({"op": "sub", "id": id, "path": path}).to_string();
        socket.send(Message::Text(frame.into())).await.expect("send");
    }

    let mut frames = Vec::new();
    while frames.len() < 2 {
        let read = tokio::time::timeout(Duration::from_secs(5), socket.next()).await;
        let Ok(Some(Ok(Message::Text(text)))) = read else {
            continue;
        };
        let frame: Value = serde_json::from_str(text.as_str()).expect("json");
        if frame.get("op").is_some_and(|op| op == "open" || op == "end") {
            frames.push(frame);
        }
    }
    assert!(frames.contains(&json!({"id": 1, "op": "open"})), "{frames:?}");
    assert!(frames.contains(&json!({"id": 2, "op": "end", "status": 415})), "{frames:?}");
}
