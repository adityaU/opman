//! A stand-in browser for tests: answers just enough CDP for a pane to open on it.
//!
//! Used by the link tests here and by the `/api/browser/device` handler tests, which put
//! it on the far end of a real websocket.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};

/// Every call the fake received, in order.
pub(crate) type CallLog = Arc<Mutex<Vec<Value>>>;

/// The reply to one call frame, or `None` for a frame that is not a call.
pub(crate) fn reply_to(frame: &Value) -> Option<Value> {
    let id = frame.get("id")?.as_i64()?;
    let method = frame.get("method")?.as_str()?;
    let result = match method {
        "Target.createTarget" => json!({ "targetId": format!("T{id}") }),
        "Target.attachToTarget" => json!({ "sessionId": format!("S{id}") }),
        "Browser.getWindowForTarget" => json!({ "windowId": 41, "bounds": {} }),
        // `window.devicePixelRatio`, as the JSON string the tab's evaluator expects.
        "Runtime.evaluate" => json!({ "result": { "type": "string", "value": "1" } }),
        _ => json!({}),
    };
    let mut reply = json!({ "id": id, "result": result });
    if let (Some(session), Some(object)) = (frame.get("sessionId"), reply.as_object_mut()) {
        object.insert("sessionId".into(), session.clone());
    }
    Some(reply)
}

/// Serve a client over channels until it goes away, logging each call.
pub(crate) async fn serve(
    mut from_client: mpsc::Receiver<String>,
    to_client: mpsc::Sender<String>,
    log: CallLog,
) {
    while let Some(text) = from_client.recv().await {
        let Ok(frame) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let reply = reply_to(&frame);
        log.lock().await.push(frame);
        let Some(reply) = reply else { continue };
        if to_client.send(reply.to_string()).await.is_err() {
            return;
        }
    }
}

/// The methods called so far, in order.
pub(crate) async fn methods(log: &CallLog) -> Vec<String> {
    log.lock()
        .await
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// The first call to `method`, if any.
pub(crate) async fn first_call(log: &CallLog, method: &str) -> Option<Value> {
    log.lock()
        .await
        .iter()
        .find(|frame| frame.get("method").and_then(Value::as_str) == Some(method))
        .cloned()
}
