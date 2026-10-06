use super::*;

use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::WebSocketStream;

type Socket = WebSocketStream<tokio::io::DuplexStream>;

/// A connected websocket pair over memory.
async fn pair() -> (Socket, Socket) {
    let (a, b) = tokio::io::duplex(1 << 16);
    let client = WebSocketStream::from_raw_socket(a, Role::Client, None).await;
    let server = WebSocketStream::from_raw_socket(b, Role::Server, None).await;
    (client, server)
}

async fn next_text(socket: &mut Socket) -> Value {
    loop {
        let message = socket
            .next()
            .await
            .expect("socket open")
            .expect("frame reads");
        if let Message::Text(text) = message {
            return serde_json::from_str(text.as_str()).expect("JSON frame");
        }
    }
}

async fn send(socket: &mut Socket, frame: Value) {
    socket
        .send(Message::Text(frame.to_string().into()))
        .await
        .expect("frame sends");
}

#[test]
fn the_ledger_learns_targets_from_create_replies() {
    let mut ledger = Ledger::default();
    ledger.outbound(r#"{"id":3,"method":"Target.createTarget","params":{}}"#);
    ledger.inbound(r#"{"method":"Page.screencastFrame","params":{}}"#);
    ledger.inbound(r#"{"id":3,"result":{"targetId":"T3"}}"#);
    assert_eq!(ledger.targets(), ["T3".to_owned()]);
}

#[test]
fn an_explicitly_closed_target_is_forgotten() {
    let mut ledger = Ledger::default();
    ledger.outbound(r#"{"id":1,"method":"Target.createTarget","params":{}}"#);
    ledger.inbound(r#"{"id":1,"result":{"targetId":"T1"}}"#);
    ledger.outbound(r#"{"id":2,"method":"Target.closeTarget","params":{"targetId":"T1"}}"#);
    assert!(ledger.targets().is_empty());
}

#[test]
fn unrelated_replies_are_ignored() {
    let mut ledger = Ledger::default();
    ledger.inbound(r#"{"id":1,"result":{"targetId":"T1"}}"#);
    assert!(ledger.targets().is_empty());
}

#[tokio::test]
async fn frames_pass_verbatim_and_windows_close_when_the_remote_leaves() {
    let (mut remote, relay_up) = pair().await;
    let (relay_local, mut browser) = pair().await;
    let relay = tokio::spawn(pipe(relay_up, relay_local));

    send(
        &mut remote,
        json!({ "id": 5, "method": "Target.createTarget", "params": {} }),
    )
    .await;
    let seen = next_text(&mut browser).await;
    assert_eq!(seen["method"], json!("Target.createTarget"));
    send(
        &mut browser,
        json!({ "id": 5, "result": { "targetId": "T5" } }),
    )
    .await;
    assert_eq!(
        next_text(&mut remote).await["result"]["targetId"],
        json!("T5")
    );

    remote.close(None).await.expect("remote closes");
    let cleanup = next_text(&mut browser).await;
    assert_eq!(cleanup["method"], json!("Target.closeTarget"));
    assert_eq!(cleanup["params"]["targetId"], json!("T5"));
    send(
        &mut browser,
        json!({ "id": cleanup["id"], "result": { "success": true } }),
    )
    .await;

    let end = relay.await.expect("relay task");
    assert!(matches!(end, Ok(RelayEnd::UpstreamClosed)));
}

#[tokio::test]
async fn a_closed_browser_ends_the_relay() {
    let (_remote, relay_up) = pair().await;
    let (relay_local, mut browser) = pair().await;
    let relay = tokio::spawn(pipe(relay_up, relay_local));
    browser.close(None).await.expect("browser closes");
    let end = relay.await.expect("relay task");
    assert!(matches!(end, Ok(RelayEnd::BrowserClosed)));
}
