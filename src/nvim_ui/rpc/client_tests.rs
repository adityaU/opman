use super::*;

use std::sync::Mutex as StdMutex;
use std::time::Duration;

use rmpv::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::super::frame::Framer;

#[derive(Default)]
struct Recorder {
    notifications: StdMutex<Vec<(String, Vec<u8>)>>,
}

impl NotificationSink for Recorder {
    fn notify(&self, method: &str, params: &[u8]) {
        self.notifications
            .lock()
            .unwrap()
            .push((method.to_owned(), params.to_vec()));
    }
}

fn wire(sink: Arc<Recorder>) -> (NvimClient, tokio::io::DuplexStream) {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    (NvimClient::new(ours, sink), theirs)
}

async fn read_value<R: AsyncRead + Unpin>(reader: &mut R) -> Value {
    let mut framer = Framer::new();
    let mut byte = [0u8; 1];
    loop {
        reader.read_exact(&mut byte).await.unwrap();
        framer.push(&byte);
        if framer.next().unwrap().is_some() {
            let mut input = framer.data();
            return rmpv::decode::read_value(&mut input).unwrap();
        }
    }
}

async fn write_value<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) {
    let mut bytes = Vec::new();
    rmpv::encode::write_value(&mut bytes, value).unwrap();
    writer.write_all(&bytes).await.unwrap();
    writer.flush().await.unwrap();
}

fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(values) => values,
        other => panic!("expected array, got {other:?}"),
    }
}

fn response(id: &Value, error: Value, result: Value) -> Value {
    Value::Array(vec![Value::from(1u64), id.clone(), error, result])
}

#[tokio::test]
async fn out_of_order_responses_correlate_by_msgid() {
    let (client, server) = wire(Arc::new(Recorder::default()));
    let (mut server_read, mut server_write) = tokio::io::split(server);
    tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..3 {
            requests.push(read_value(&mut server_read).await);
        }
        for request in requests.iter().rev() {
            let fields = array(request);
            write_value(
                &mut server_write,
                &response(&fields[1], Value::Nil, fields[2].clone()),
            )
            .await;
        }
    });

    let first = client.request("one", Value::Array(Vec::new()));
    let second = client.request("two", Value::Array(Vec::new()));
    let third = client.request("three", Value::Array(Vec::new()));
    let (first, second, third) = tokio::join!(first, second, third);
    assert_eq!(first.unwrap(), Value::from("one"));
    assert_eq!(second.unwrap(), Value::from("two"));
    assert_eq!(third.unwrap(), Value::from("three"));
}

#[tokio::test]
async fn error_response_is_an_err() {
    let (client, mut server) = wire(Arc::new(Recorder::default()));
    let request = tokio::spawn({
        let client = client.clone();
        async move { client.request("missing", Value::Array(Vec::new())).await }
    });
    let frame = read_value(&mut server).await;
    let id = &array(&frame)[1];
    write_value(
        &mut server,
        &response(id, Value::from("method not found"), Value::Nil),
    )
    .await;
    let error = request.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("method not found"));
}

#[tokio::test]
async fn unknown_msgid_is_ignored() {
    let (client, mut server) = wire(Arc::new(Recorder::default()));
    let request = tokio::spawn({
        let client = client.clone();
        async move { client.request("ok", Value::Array(Vec::new())).await }
    });
    let frame = read_value(&mut server).await;
    let id = &array(&frame)[1];
    write_value(
        &mut server,
        &response(&Value::from(999_999u64), Value::Nil, Value::from("bad")),
    )
    .await;
    write_value(&mut server, &response(id, Value::Nil, Value::from("good"))).await;
    assert_eq!(request.await.unwrap().unwrap(), Value::from("good"));
}

#[tokio::test]
async fn notifications_reach_the_sink_with_encoded_params() {
    let sink = Arc::new(Recorder::default());
    let (client, mut server) = wire(sink.clone());
    write_value(
        &mut server,
        &Value::Array(vec![
            Value::from(2u64),
            Value::from("redraw"),
            Value::Array(vec![Value::Array(vec![Value::from("grid_line")])]),
        ]),
    )
    .await;
    for _ in 0..50 {
        if !sink.notifications.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let notifications = sink.notifications.lock().unwrap();
    assert_eq!(notifications[0].0, "redraw");
    assert_eq!(
        notifications[0].1,
        vec![0x91, 0x91, 0xa9, b'g', b'r', b'i', b'd', b'_', b'l', b'i', b'n', b'e']
    );
    drop(client);
}

#[tokio::test]
async fn eof_fails_all_outstanding_waiters() {
    let (client, server) = wire(Arc::new(Recorder::default()));
    let (mut server_read, server_write) = tokio::io::split(server);
    tokio::spawn(async move {
        for _ in 0..3 {
            let _ = read_value(&mut server_read).await;
        }
        drop(server_write);
    });
    let a = client.request("a", Value::Array(Vec::new()));
    let b = client.request("b", Value::Array(Vec::new()));
    let c = client.request("c", Value::Array(Vec::new()));
    let (a, b, c) = tokio::join!(a, b, c);
    assert!(a.is_err() && b.is_err() && c.is_err());
    assert!(!client.is_alive());
}

#[tokio::test]
async fn timeout_removes_only_its_own_waiter() {
    let (client, mut server) = wire(Arc::new(Recorder::default()));
    let slow = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .request_timeout("slow", Value::Array(Vec::new()), Duration::from_millis(20))
                .await
        }
    });
    let other = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .request_timeout("other", Value::Array(Vec::new()), Duration::from_secs(1))
                .await
        }
    });
    let first = read_value(&mut server).await;
    let second = read_value(&mut server).await;
    let other_id = if array(&first)[2] == Value::from("other") {
        array(&first)[1].clone()
    } else {
        array(&second)[1].clone()
    };
    tokio::time::sleep(Duration::from_millis(40)).await;
    write_value(
        &mut server,
        &response(&other_id, Value::Nil, Value::from("still alive")),
    )
    .await;
    assert!(slow.await.unwrap().is_err());
    assert_eq!(other.await.unwrap().unwrap(), Value::from("still alive"));
}

#[tokio::test]
async fn inbound_unknown_request_receives_an_error_reply() {
    let (client, mut server) = wire(Arc::new(Recorder::default()));
    write_value(
        &mut server,
        &Value::Array(vec![
            Value::from(0u64),
            Value::from(7u64),
            Value::from("rpcrequest"),
            Value::Array(Vec::new()),
        ]),
    )
    .await;
    let reply = read_value(&mut server).await;
    let fields = array(&reply);
    assert_eq!(fields[0], Value::from(1u64));
    assert_eq!(fields[1], Value::from(7u64));
    assert!(!matches!(fields[2], Value::Nil));
    assert!(matches!(fields[3], Value::Nil));
    drop(client);
}

#[tokio::test]
async fn malformed_frame_terminates_reader_and_fails_request() {
    let (client, mut server) = wire(Arc::new(Recorder::default()));
    server.write_all(&[0xc1]).await.unwrap();
    let error = client
        .request_timeout("never", Value::Array(Vec::new()), Duration::from_secs(1))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("malformed") || error.to_string().contains("closed"));
    assert!(!client.is_alive());
}
