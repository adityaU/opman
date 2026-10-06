use super::*;

fn pending() -> Pending {
    Arc::new(Mutex::new(HashMap::new()))
}

#[tokio::test]
async fn a_result_reaches_the_waiting_call() {
    let pending = pending();
    let (events, _rx) = broadcast::channel(8);
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(7, tx);

    route(
        json!({ "id": 7, "result": { "ok": true } }),
        &pending,
        &events,
    )
    .await;

    let value = rx.await.expect("reply arrives").expect("not an error");
    assert_eq!(value, json!({ "ok": true }));
    assert!(pending.lock().await.is_empty(), "the entry is consumed");
}

#[tokio::test]
async fn an_error_reply_carries_the_data_field() {
    let pending = pending();
    let (events, _rx) = broadcast::channel(8);
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(1, tx);

    let frame = json!({
        "id": 1,
        "error": { "code": -32000, "message": "Cannot find context", "data": "for id 4" },
    });
    route(frame, &pending, &events).await;

    let error = rx
        .await
        .expect("reply arrives")
        .expect_err("must be an error");
    assert_eq!(error, "Cannot find context: for id 4");
}

#[tokio::test]
async fn a_result_with_no_result_field_is_null_not_an_error() {
    let pending = pending();
    let (events, _rx) = broadcast::channel(8);
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(3, tx);

    route(json!({ "id": 3 }), &pending, &events).await;

    assert_eq!(rx.await.expect("reply").expect("ok"), Value::Null);
}

#[tokio::test]
async fn events_fan_out_with_their_session() {
    let pending = pending();
    let (events, mut rx) = broadcast::channel(8);

    let frame = json!({
        "method": "Page.screencastFrame",
        "sessionId": "S1",
        "params": { "data": "abc" },
    });
    route(frame, &pending, &events).await;

    let event = rx.try_recv().expect("an event was published");
    assert_eq!(&*event.method, "Page.screencastFrame");
    assert_eq!(event.session_id.as_deref(), Some("S1"));
    assert_eq!(
        event.params.get("data").and_then(Value::as_str),
        Some("abc")
    );
}

#[tokio::test]
async fn a_reply_for_an_unknown_id_is_dropped_quietly() {
    let pending = pending();
    let (events, mut rx) = broadcast::channel(8);

    route(json!({ "id": 99, "result": {} }), &pending, &events).await;

    assert!(rx.try_recv().is_err(), "a late reply is not an event");
}

/// A fake browser on the far end of [`Cdp::from_channels`]: answers every call with its own
/// method name, and publishes one event per call.
#[tokio::test]
async fn a_client_over_channels_calls_and_subscribes() {
    let (outgoing, mut to_peer) = mpsc::channel::<String>(8);
    let (from_peer, incoming) = mpsc::channel::<String>(8);
    tokio::spawn(async move {
        while let Some(text) = to_peer.recv().await {
            let frame: Value = serde_json::from_str(&text).expect("client sends JSON");
            let event =
                json!({ "method": "Fake.called", "sessionId": frame["sessionId"], "params": {} });
            let _ = from_peer.send(event.to_string()).await;
            let reply = json!({ "id": frame["id"], "result": { "echo": frame["method"] } });
            let _ = from_peer.send(reply.to_string()).await;
        }
    });

    let cdp = Cdp::from_channels(outgoing, incoming);
    let mut events = cdp.subscribe();
    let reply = cdp
        .call_on("S9", "Page.enable", json!({}))
        .await
        .expect("answered");
    assert_eq!(reply, json!({ "echo": "Page.enable" }));

    let event = events.recv().await.expect("event delivered");
    assert_eq!(&*event.method, "Fake.called");
    assert_eq!(event.session_id.as_deref(), Some("S9"));
}

#[tokio::test]
async fn a_peer_that_goes_away_fails_calls_instead_of_hanging() {
    let (outgoing, to_peer) = mpsc::channel::<String>(8);
    let (from_peer, incoming) = mpsc::channel::<String>(8);
    drop(to_peer);
    drop(from_peer);

    let cdp = Cdp::from_channels(outgoing, incoming);
    let started = std::time::Instant::now();
    let error = cdp.call("Browser.getVersion", json!({})).await;
    assert!(error.is_err());
    assert!(started.elapsed() < Duration::from_secs(5), "failed fast");
}
