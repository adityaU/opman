//! One stream socket: reads sub/unsub frames, runs a task per subscription, and writes
//! their frames plus keepalives back to the client.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::http::{HeaderMap, StatusCode};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior};

use super::protocol::{self, ClientFrame};
use super::{subscription, RouterSlot};

/// How often the server pings — both a WebSocket ping and a `{"op":"ping"}` frame the
/// page can see, since browsers answer pings without telling the page.
const PING_EVERY: Duration = Duration::from_secs(15);
/// A client silent this long (not even a pong) is gone.
const SILENCE_LIMIT: Duration = Duration::from_secs(60);
/// Frames waiting for the socket. Small on purpose: a slow client slows its streams
/// instead of growing a queue.
const OUTBOX: usize = 16;
/// Streams one socket may hold at once.
const MAX_SUBSCRIPTIONS: usize = 256;

/// Live subscription tasks by id. Dropping it aborts them all, which drops each
/// internal response body and so releases the endpoint's resources.
#[derive(Default)]
struct Subscriptions(HashMap<u32, JoinHandle<()>>);

impl Subscriptions {
    fn start(&mut self, id: u32, task: impl FnOnce() -> JoinHandle<()>) -> bool {
        self.stop(id);
        self.0.retain(|_, handle| !handle.is_finished());
        if self.0.len() >= MAX_SUBSCRIPTIONS {
            return false;
        }
        self.0.insert(id, task());
        true
    }

    fn stop(&mut self, id: u32) {
        if let Some(handle) = self.0.remove(&id) {
            handle.abort();
        }
    }
}

impl Drop for Subscriptions {
    fn drop(&mut self) {
        for handle in self.0.values() {
            handle.abort();
        }
    }
}

pub(super) async fn run(socket: WebSocket, slot: RouterSlot, credentials: Arc<HeaderMap>) {
    let (mut sink, mut source) = socket.split();
    let (out, mut outbox) = mpsc::channel::<String>(OUTBOX);
    let mut subscriptions = Subscriptions::default();
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut heard = Instant::now();

    loop {
        tokio::select! {
            incoming = source.next() => {
                let Some(Ok(message)) = incoming else { break };
                heard = Instant::now();
                match message {
                    Message::Text(text) => {
                        let Ok(frame) = serde_json::from_str::<ClientFrame>(text.as_str()) else {
                            continue;
                        };
                        handle(frame, &mut subscriptions, &slot, &credentials, &out);
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            Some(frame) = outbox.recv() => {
                if sink.send(Message::Text(frame.into())).await.is_err() {
                    break;
                }
            }
            _ = ping.tick() => {
                if heard.elapsed() > SILENCE_LIMIT {
                    break;
                }
                if sink.send(Message::Ping(Bytes::new())).await.is_err()
                    || sink.send(Message::Text(protocol::ping().into())).await.is_err()
                {
                    break;
                }
            }
        }
    }
}

fn handle(
    frame: ClientFrame,
    subscriptions: &mut Subscriptions,
    slot: &RouterSlot,
    credentials: &Arc<HeaderMap>,
    out: &mpsc::Sender<String>,
) {
    let (id, path) = match frame {
        ClientFrame::Unsub { id } => return subscriptions.stop(id),
        ClientFrame::Sub { id, path } => (id, path),
    };
    let started = subscriptions.start(id, || {
        tokio::spawn(subscription::pump(
            id,
            path,
            slot.clone(),
            credentials.clone(),
            out.clone(),
        ))
    });
    if started {
        return;
    }
    // Refusing must not wait on the outbox from inside the read loop.
    let refused = protocol::end(id, Some(StatusCode::TOO_MANY_REQUESTS));
    let out = out.clone();
    tokio::spawn(async move {
        let _ = out.send(refused).await;
    });
}
