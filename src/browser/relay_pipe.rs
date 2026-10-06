//! The frame pump behind [`super::relay::run_device_link`].
//!
//! Frames go through untouched. The only thing read out of them is which windows the
//! remote created — `Target.createTarget` calls and their replies — so they can be closed
//! when the link ends. Screencast frames are large and frequent, so a frame is parsed only
//! when it could possibly be one of those two.

// Unreferenced until the remote-server supervisor starts a relay per configured remote.
#![cfg_attr(not(test), allow(dead_code))]

use std::collections::HashSet;
use std::time::Duration;

use futures::{Sink, SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use super::relay::{RelayEnd, RelayError};

/// How long cleanup waits for the browser to confirm closed windows.
const CLEANUP_GRACE: Duration = Duration::from_secs(2);
/// Ids for the relay's own cleanup calls, far above anything a remote's counter reaches,
/// so a late reply can never be mistaken for one of the remote's.
const CLEANUP_ID_BASE: i64 = 1 << 52;

/// Windows the remote created through this relay.
#[derive(Debug, Default)]
pub(super) struct Ledger {
    creating: HashSet<i64>,
    targets: Vec<String>,
}

impl Ledger {
    /// A frame from the remote: note `Target.createTarget` calls and explicit closes.
    pub fn outbound(&mut self, text: &str) {
        if !text.contains("Target.") {
            return;
        }
        let Ok(frame) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match frame.get("method").and_then(Value::as_str) {
            Some("Target.createTarget") => {
                if let Some(id) = frame.get("id").and_then(Value::as_i64) {
                    self.creating.insert(id);
                }
            }
            Some("Target.closeTarget") => {
                let closed = frame
                    .get("params")
                    .and_then(|params| params.get("targetId"))
                    .and_then(Value::as_str);
                if let Some(closed) = closed {
                    self.targets.retain(|target| target != closed);
                }
            }
            _ => {}
        }
    }

    /// A frame from the browser: if it answers a pending create, remember the target.
    pub fn inbound(&mut self, text: &str) {
        // Replies lead with their id; events lead with their method.
        if self.creating.is_empty() || !text.starts_with("{\"id\":") {
            return;
        }
        let Ok(frame) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let Some(id) = frame.get("id").and_then(Value::as_i64) else {
            return;
        };
        if !self.creating.remove(&id) {
            return;
        }
        let target = frame
            .get("result")
            .and_then(|result| result.get("targetId"))
            .and_then(Value::as_str);
        if let Some(target) = target {
            self.targets.push(target.to_owned());
        }
    }

    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

/// Copy frames both ways until one side closes, then close the remote's windows.
pub(super) async fn pipe<U, L>(upstream: U, local: L) -> Result<RelayEnd, RelayError>
where
    U: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
    L: Stream<Item = Result<Message, WsError>> + Sink<Message, Error = WsError> + Unpin,
{
    let (mut up_tx, mut up_rx) = upstream.split();
    let (mut local_tx, mut local_rx) = local.split();
    let mut ledger = Ledger::default();

    let end = loop {
        tokio::select! {
            frame = up_rx.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    ledger.outbound(text.as_str());
                    if local_tx.send(Message::Text(text)).await.is_err() {
                        break Ok(RelayEnd::BrowserClosed);
                    }
                }
                Some(Ok(Message::Binary(bytes))) => {
                    if local_tx.send(Message::Binary(bytes)).await.is_err() {
                        break Ok(RelayEnd::BrowserClosed);
                    }
                }
                Some(Ok(Message::Close(_))) | None => break Ok(RelayEnd::UpstreamClosed),
                // Pings are answered by the websocket layer itself.
                Some(Ok(_)) => {}
                Some(Err(error)) => break Err(RelayError::Socket(error)),
            },
            frame = local_rx.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    ledger.inbound(text.as_str());
                    if let Err(error) = up_tx.send(Message::Text(text)).await {
                        break Err(RelayError::Socket(error));
                    }
                }
                Some(Ok(Message::Binary(bytes))) => {
                    if let Err(error) = up_tx.send(Message::Binary(bytes)).await {
                        break Err(RelayError::Socket(error));
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                    break Ok(RelayEnd::BrowserClosed);
                }
                Some(Ok(_)) => {}
            },
        }
    };

    let _ = up_tx.close().await;
    if end.as_ref().ok() != Some(&RelayEnd::BrowserClosed) {
        close_targets(&mut local_tx, &mut local_rx, ledger.targets()).await;
    }
    let _ = local_tx.close().await;
    end
}

/// Close each window over the relay's own browser connection and wait (briefly) for the
/// browser to confirm, so the socket is not dropped with the calls still unread.
async fn close_targets<Tx, Rx>(tx: &mut Tx, rx: &mut Rx, targets: &[String])
where
    Tx: Sink<Message, Error = WsError> + Unpin,
    Rx: Stream<Item = Result<Message, WsError>> + Unpin,
{
    let mut waiting = HashSet::new();
    for (offset, target) in (0_i64..).zip(targets) {
        let id = CLEANUP_ID_BASE + offset;
        let call =
            json!({ "id": id, "method": "Target.closeTarget", "params": { "targetId": target } });
        if tx
            .send(Message::Text(call.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
        waiting.insert(id);
    }
    let confirmations = async {
        while !waiting.is_empty() {
            let Some(Ok(message)) = rx.next().await else {
                return;
            };
            let Message::Text(text) = message else {
                continue;
            };
            let id = serde_json::from_str::<Value>(text.as_str())
                .ok()
                .and_then(|frame| frame.get("id").and_then(Value::as_i64));
            if let Some(id) = id {
                waiting.remove(&id);
            }
        }
    };
    let _ = tokio::time::timeout(CLEANUP_GRACE, confirmations).await;
}

#[cfg(test)]
#[path = "relay_pipe_tests.rs"]
mod relay_pipe_tests;
