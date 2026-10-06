//! `GET /api/browser/device` — where a device browser attaches to this server.
//!
//! The far end is another opman's relay (see `crate::browser::run_device_link`): text
//! frames are raw CDP JSON both ways. While the link is up, this server's browser panes
//! open in that device's browser instead of a local Chromium. The newest link wins; a
//! replaced link is closed with a reason so its relay can tell why.

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use super::super::auth::AuthUser;
use super::super::types::ServerState;
use crate::browser::{BrowserPool, DeviceLink};

/// Application close code for "a newer device link took over".
const SUPERSEDED: u16 = 4000;

pub async fn browser_device(
    State(state): State<ServerState>,
    _auth: AuthUser,
    upgrade: WebSocketUpgrade,
) -> Response {
    let pool = state.browser.clone();
    upgrade.on_upgrade(move |socket| serve(pool, socket))
}

/// Pump one link until either side ends it, then detach it from the pool.
async fn serve(pool: BrowserPool, socket: WebSocket) {
    let DeviceLink {
        id,
        mut to_device,
        from_device,
        mut superseded,
    } = pool.attach_link().await;
    let (mut sink, mut source) = socket.split();

    loop {
        tokio::select! {
            // Replacing a link drops both its signal and its frame channel at once; checking
            // the signal first is what gets the replaced relay a reason rather than a reset.
            biased;
            _ = &mut superseded => {
                close_superseded(&mut sink).await;
                break;
            }
            outgoing = to_device.recv() => {
                let Some(text) = outgoing else {
                    close_superseded(&mut sink).await;
                    break;
                };
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = source.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if from_device.send(text.as_str().to_owned()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    pool.detach_link(id).await;
}

/// Tell the far end it was replaced — the only way the pool lets go of a live link.
async fn close_superseded(sink: &mut futures::stream::SplitSink<WebSocket, Message>) {
    let frame = CloseFrame {
        code: SUPERSEDED,
        reason: "replaced by a newer device link".into(),
    };
    let _ = sink.send(Message::Close(Some(frame))).await;
}

#[cfg(test)]
#[path = "browser_device_tests.rs"]
mod browser_device_tests;
