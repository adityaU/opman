//! The per-connection session: one socket, many requests in flight.
//!
//! Each request runs as its own task and answers on a shared outbound channel,
//! so a slow completion never holds up the hover behind it. That is the point
//! of the channel over one-request-per-connection HTTP: a browser gives an
//! origin about six sockets, and a fast scroll or a fast typist can put more
//! than six queries in the air at once — past which they queue behind each
//! other in the connection pool rather than at the language server.
//!
//! Cancellation is the other half. A hover the pointer has already moved past
//! is work nobody will read; aborting it frees the server for the one the
//! reader is waiting on. Writes are never abandoned mid-flight.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::web::types::ServerState;

use super::dispatch;
use super::protocol::{decode, encode, Op, Request, Response};

type Outbound = Arc<Mutex<SplitSink<WebSocket, Message>>>;

/// In-flight requests, so a cancel has something to abort.
#[derive(Default)]
struct InFlight {
    tasks: HashMap<u64, JoinHandle<()>>,
}

impl InFlight {
    fn insert(&mut self, id: u64, task: JoinHandle<()>) {
        self.tasks.insert(id, task);
    }

    fn finish(&mut self, id: u64) {
        self.tasks.remove(&id);
    }

    /// Abort one request. Returns whether there was anything to abort.
    fn cancel(&mut self, id: u64) -> bool {
        match self.tasks.remove(&id) {
            Some(task) => {
                task.abort();
                true
            }
            None => false,
        }
    }

    fn abort_all(&mut self) {
        for (_, task) in self.tasks.drain() {
            task.abort();
        }
    }
}

pub async fn run(socket: WebSocket, state: ServerState) {
    let (sender, mut receiver) = socket.split();
    let outbound: Outbound = Arc::new(Mutex::new(sender));
    let in_flight = Arc::new(Mutex::new(InFlight::default()));

    while let Some(message) = receiver.next().await {
        let bytes = match message {
            Ok(Message::Binary(bytes)) => bytes,
            Ok(Message::Close(_)) => break,
            // Text frames are not part of this protocol; a client sending them
            // is a client talking to the wrong endpoint.
            Ok(_) => continue,
            Err(error) => {
                debug!("editor ws: receive failed: {error}");
                break;
            }
        };

        let request = match decode(&bytes) {
            Ok(request) => request,
            Err(error) => {
                debug!("editor ws: undecodable frame: {error}");
                continue;
            }
        };

        if request.op == Op::Cancel {
            handle_cancel(&in_flight, &outbound, request).await;
            continue;
        }

        spawn_request(&state, &outbound, &in_flight, request);
    }

    in_flight.lock().await.abort_all();
}

/// A cancel is answered too: the client can free its pending entry without
/// waiting to find out whether the abort beat the response.
async fn handle_cancel(in_flight: &Arc<Mutex<InFlight>>, outbound: &Outbound, request: Request) {
    let target = request
        .payload
        .get("target")
        .and_then(serde_json::Value::as_u64);
    let aborted = match target {
        Some(id) => in_flight.lock().await.cancel(id),
        None => false,
    };
    send(outbound, &Response::ok(request.id, serde_json::json!({ "cancelled": aborted }))).await;
}

fn spawn_request(
    state: &ServerState,
    outbound: &Outbound,
    in_flight: &Arc<Mutex<InFlight>>,
    request: Request,
) {
    let state = state.clone();
    let outbound = Arc::clone(outbound);
    let tracker = Arc::clone(in_flight);
    let id = request.id;
    let cancellable = request.op.is_read_only();

    let task = tokio::spawn(async move {
        let response = match dispatch::run(&state, request.op, request.payload).await {
            Ok(result) => Response::ok(id, result),
            Err(error) => Response::failed(id, error),
        };
        send(&outbound, &response).await;
        tracker.lock().await.finish(id);
    });

    // A write that has already started must be allowed to finish, so it is
    // never put where a cancel could reach it.
    if !cancellable {
        return;
    }
    let tracker = Arc::clone(in_flight);
    tokio::spawn(async move {
        tracker.lock().await.insert(id, task);
    });
}

async fn send<T: serde::Serialize>(outbound: &Outbound, value: &T) {
    let Ok(bytes) = encode(value) else {
        debug!("editor ws: could not encode a response");
        return;
    };
    let mut sink = outbound.lock().await;
    if let Err(error) = sink.send(Message::Binary(bytes.into())).await {
        debug!("editor ws: send failed: {error}");
    }
}
