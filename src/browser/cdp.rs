//! A small Chrome DevTools Protocol client over the browser-level websocket.
//!
//! Flat session mode (`Target.attachToTarget { flatten: true }`) is what keeps this
//! small: every page multiplexes onto the one socket and is addressed by `sessionId`,
//! so there is a single reader task, a single pending-call map, and no per-tab
//! connection to leak. Calls are correlated by monotonic id; events fan out on a
//! broadcast so several consumers (screencast, load-waiter) can watch one page.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;

/// Ceiling on any single CDP call. A page that never answers `Page.navigate` must not
/// wedge the tool call that asked for it.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Event fan-out depth. Screencast frames are the fast producer; a slow consumer lags
/// rather than blocking the reader.
const EVENT_CHANNEL: usize = 256;

/// A CDP event, already split into the page it came from.
#[derive(Clone, Debug)]
pub struct CdpEvent {
    /// `None` for browser-level events.
    pub session_id: Option<Arc<str>>,
    pub method: Arc<str>,
    pub params: Arc<Value>,
}

/// Outbound work for the writer half.
struct Call {
    payload: Value,
    reply: oneshot::Sender<Result<Value, String>>,
    id: i64,
}

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>>;

/// Connected DevTools client. Cloneable; every clone shares the one socket.
#[derive(Clone)]
pub struct Cdp {
    tx: mpsc::UnboundedSender<Call>,
    events: broadcast::Sender<CdpEvent>,
    next_id: Arc<AtomicI64>,
}

impl Cdp {
    /// Dial the endpoint printed by [`super::chrome::Chrome`] and start the pump.
    pub async fn connect(ws_url: &str) -> anyhow::Result<Self> {
        let (stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
        let (mut sink, mut source) = stream.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<Call>();
        let (events, _) = broadcast::channel(EVENT_CHANNEL);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));

        let writer_pending = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(call) = rx.recv().await {
                let text = call.payload.to_string();
                writer_pending.lock().await.insert(call.id, call.reply);
                if sink.send(Message::Text(text.into())).await.is_err() {
                    if let Some(reply) = writer_pending.lock().await.remove(&call.id) {
                        let _ = reply.send(Err("devtools socket closed".into()));
                    }
                    break;
                }
            }
        });

        let reader_events = events.clone();
        let reader_pending = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(Ok(msg)) = source.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                route(value, &reader_pending, &reader_events).await;
            }
            // Socket died: fail every waiter rather than leaving them on the timeout.
            for (_, reply) in reader_pending.lock().await.drain() {
                let _ = reply.send(Err("devtools socket closed".into()));
            }
        });

        Ok(Self {
            tx,
            events,
            next_id: Arc::new(AtomicI64::new(1)),
        })
    }

    /// Subscribe to every event on every page. Filter by `session_id` at the consumer.
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// Invoke a browser-level method.
    pub async fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.dispatch(method, params, None).await
    }

    /// Invoke a method against one attached page.
    pub async fn call_on(
        &self,
        session_id: &str,
        method: &str,
        params: Value,
    ) -> anyhow::Result<Value> {
        self.dispatch(method, params, Some(session_id)).await
    }

    /// Fire and forget — used for screencast frame acks, where a reply would only add
    /// latency to the next frame.
    pub fn notify_on(&self, session_id: &str, method: &str, params: Value) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, _drop) = oneshot::channel();
        let _ = self.tx.send(Call {
            payload: json!({ "id": id, "method": method, "params": params, "sessionId": session_id }),
            reply,
            id,
        });
    }

    async fn dispatch(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> anyhow::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut payload = json!({ "id": id, "method": method, "params": params });
        if let (Some(session), Some(obj)) = (session_id, payload.as_object_mut()) {
            obj.insert("sessionId".into(), Value::String(session.to_owned()));
        }

        let (reply, wait) = oneshot::channel();
        self.tx
            .send(Call { payload, reply, id })
            .map_err(|_| anyhow::anyhow!("devtools client is shut down"))?;

        match tokio::time::timeout(CALL_TIMEOUT, wait).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(e))) => Err(anyhow::anyhow!("{method} failed: {e}")),
            Ok(Err(_)) => Err(anyhow::anyhow!("{method} was dropped before replying")),
            Err(_) => Err(anyhow::anyhow!(
                "{method} timed out after {}s",
                CALL_TIMEOUT.as_secs()
            )),
        }
    }
}

/// Send one inbound frame either to the call that is waiting for it or to subscribers.
async fn route(mut value: Value, pending: &Pending, events: &broadcast::Sender<CdpEvent>) {
    let session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .map(Arc::<str>::from);

    if let Some(id) = value.get("id").and_then(Value::as_i64) {
        let Some(reply) = pending.lock().await.remove(&id) else {
            return;
        };
        let outcome = match value.get_mut("error") {
            Some(error) => Err(error_message(error)),
            None => Ok(value
                .get_mut("result")
                .map(Value::take)
                .unwrap_or(Value::Null)),
        };
        let _ = reply.send(outcome);
        return;
    }

    let Some(method) = value.get("method").and_then(Value::as_str).map(Arc::from) else {
        return;
    };
    let params = value
        .get_mut("params")
        .map(Value::take)
        .unwrap_or(Value::Null);
    // `send` errors only when nobody is subscribed, which is the normal idle state.
    let _ = events.send(CdpEvent {
        session_id,
        method,
        params: Arc::new(params),
    });
}

/// CDP errors carry `{code, message, data}`; `data` holds the useful half for
/// evaluation failures, so keep it when present.
fn error_message(error: &mut Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown devtools error");
    match error.get("data").and_then(Value::as_str) {
        Some(data) => format!("{message}: {data}"),
        None => message.to_owned(),
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod cdp_tests;
