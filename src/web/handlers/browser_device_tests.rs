//! The device-link endpoint end to end: a real websocket, a fake browser behind it, and
//! the pool opening a pane over it.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::browser::fake_device;
use crate::browser::{LinkState, Opened};
use crate::web::test_support::test_server_state;

type Client =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Serve the full router on a loopback port and return its address.
async fn serve(state: crate::web::ServerState) -> std::net::SocketAddr {
    let router = crate::web::routes::build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

async fn dial(addr: std::net::SocketAddr) -> Client {
    let url = format!("ws://{addr}/api/browser/device");
    let (socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("upgrade");
    socket
}

/// Answer every CDP call arriving on the socket, like a browser would.
fn act_as_browser(socket: Client, log: fake_device::CallLog) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (mut sink, mut source) = socket.split();
        while let Some(Ok(message)) = source.next().await {
            let WsMessage::Text(text) = message else {
                continue;
            };
            let Ok(frame) = serde_json::from_str::<Value>(text.as_str()) else {
                continue;
            };
            let reply = fake_device::reply_to(&frame);
            log.lock().await.push(frame);
            if let Some(reply) = reply {
                if sink
                    .send(WsMessage::Text(reply.to_string().into()))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
    })
}

async fn wait_for(state: &crate::web::ServerState, wanted: LinkState) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while state.browser.status().await.device_link != wanted {
        assert!(
            tokio::time::Instant::now() < deadline,
            "link never became {wanted:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_device_link_carries_the_panes_and_takes_them_when_it_drops() {
    let state = test_server_state();
    let addr = serve(state.clone()).await;
    let log = fake_device::CallLog::default();
    let browser = act_as_browser(dial(addr).await, log.clone());
    wait_for(&state, LinkState::Attached).await;

    let (_, opened) = state
        .browser
        .open("proj:/p", "/p")
        .await
        .expect("opens over the link");
    assert_eq!(opened, Opened::Created);
    let methods = fake_device::methods(&log).await;
    assert!(methods.iter().any(|m| m == "Target.createTarget"));
    assert!(methods.iter().any(|m| m == "Browser.setWindowBounds"));

    let listed = super::super::browser_ops::list(&state)
        .await
        .expect("lists");
    assert_eq!(listed["browser"]["device_link"], "attached");
    assert_eq!(listed["panes"].as_array().map(Vec::len), Some(1));

    browser.abort();
    wait_for(&state, LinkState::Detached).await;
    assert!(state.browser.list().await.is_empty());
}

#[tokio::test]
async fn a_newer_link_closes_the_older_one() {
    let state = test_server_state();
    let addr = serve(state.clone()).await;
    let mut first = dial(addr).await;
    wait_for(&state, LinkState::Attached).await;
    let _second = dial(addr).await;

    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match first.next().await {
                Some(Ok(WsMessage::Close(frame))) => return frame.map(|f| u16::from(f.code)),
                Some(Ok(_)) => continue,
                _ => return None,
            }
        }
    })
    .await
    .expect("the old link is closed promptly");
    assert_eq!(closed, Some(4000));
    assert_eq!(
        state.browser.status().await.device_link,
        LinkState::Attached
    );
}
