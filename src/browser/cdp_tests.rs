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

    route(json!({ "id": 7, "result": { "ok": true } }), &pending, &events).await;

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

    let error = rx.await.expect("reply arrives").expect_err("must be an error");
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
    assert_eq!(event.params.get("data").and_then(Value::as_str), Some("abc"));
}

#[tokio::test]
async fn a_reply_for_an_unknown_id_is_dropped_quietly() {
    let pending = pending();
    let (events, mut rx) = broadcast::channel(8);

    route(json!({ "id": 99, "result": {} }), &pending, &events).await;

    assert!(rx.try_recv().is_err(), "a late reply is not an event");
}
