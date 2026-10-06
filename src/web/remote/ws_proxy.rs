//! WebSocket upgrades on `/remote/{id}/api/*`: connect upstream first, then accept the
//! browser and pump frames both ways until either side closes.
//!
//! Connecting first means a dead or refusing remote is an HTTP 502 the browser can see,
//! not an accepted socket that closes at once.

use axum::body::Bytes;
use axum::extract::ws::{self as ax, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame as TsCloseFrame;
use tokio_tungstenite::tungstenite::{Message as TsMessage, Utf8Bytes as TsUtf8};

use super::{gateway_error, WebError};
use crate::remote::{Registry, RemoteWs, ServerId};

pub async fn accept(
    upgrade: WebSocketUpgrade,
    registry: &Registry,
    id: &ServerId,
    path: &str,
) -> Result<Response, WebError> {
    let upstream = registry
        .connect_ws(id, path)
        .await
        .map_err(gateway_error)?;
    Ok(upgrade.on_upgrade(move |client| pump(client, upstream)))
}

async fn pump(client: WebSocket, upstream: RemoteWs) {
    let (mut client_tx, mut client_rx) = client.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();
    let to_upstream = async {
        while let Some(Ok(message)) = client_rx.next().await {
            let Some(message) = into_upstream(message) else {
                continue;
            };
            let closing = message.is_close();
            if upstream_tx.send(message).await.is_err() || closing {
                break;
            }
        }
        let _ = upstream_tx.close().await;
    };
    let to_client = async {
        while let Some(Ok(message)) = upstream_rx.next().await {
            let Some(message) = into_client(message) else {
                continue;
            };
            let closing = matches!(message, ax::Message::Close(_));
            if client_tx.send(message).await.is_err() || closing {
                break;
            }
        }
        let _ = client_tx.close().await;
    };
    tokio::select! {
        () = to_upstream => {}
        () = to_client => {}
    }
}

fn into_upstream(message: ax::Message) -> Option<TsMessage> {
    Some(match message {
        ax::Message::Text(text) => TsMessage::Text(TsUtf8::try_from(Bytes::from(text)).ok()?),
        ax::Message::Binary(bytes) => TsMessage::Binary(bytes),
        ax::Message::Ping(bytes) => TsMessage::Ping(bytes),
        ax::Message::Pong(bytes) => TsMessage::Pong(bytes),
        ax::Message::Close(frame) => TsMessage::Close(frame.map(|f| TsCloseFrame {
            code: CloseCode::from(f.code),
            reason: TsUtf8::try_from(Bytes::from(f.reason)).unwrap_or_default(),
        })),
    })
}

fn into_client(message: TsMessage) -> Option<ax::Message> {
    Some(match message {
        TsMessage::Text(text) => {
            ax::Message::Text(ax::Utf8Bytes::try_from(Bytes::from(text)).ok()?)
        }
        TsMessage::Binary(bytes) => ax::Message::Binary(bytes),
        TsMessage::Ping(bytes) => ax::Message::Ping(bytes),
        TsMessage::Pong(bytes) => ax::Message::Pong(bytes),
        TsMessage::Close(frame) => ax::Message::Close(frame.map(|f| ax::CloseFrame {
            code: u16::from(f.code),
            reason: ax::Utf8Bytes::try_from(Bytes::from(f.reason)).unwrap_or_default(),
        })),
        // Raw frames only appear when writing; a read never yields one.
        TsMessage::Frame(_) => return None,
    })
}
