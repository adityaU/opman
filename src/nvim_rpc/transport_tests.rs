use super::{checked_value, nvim_call, parse_response, IncomingResponse};
use rmpv::Value;
use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::thread;

fn response(msgid: u64, result: Value) -> Value {
    Value::Array(vec![
        Value::from(1u64),
        Value::from(msgid),
        Value::Nil,
        result,
    ])
}

#[test]
fn wrong_msgid_is_skipped() {
    let parsed = parse_response(response(41, Value::from("wrong")), 42).expect("valid frame");
    assert!(matches!(parsed, IncomingResponse::Skip));
}

#[test]
fn notification_is_skipped_before_matching_response() {
    let notification = Value::Array(vec![
        Value::from(2u64),
        Value::from("redraw"),
        Value::Array(vec![]),
    ]);
    assert!(matches!(
        parse_response(notification, 42).expect("valid notification"),
        IncomingResponse::Skip
    ));

    let parsed = parse_response(response(42, Value::from("right")), 42).expect("valid response");
    assert!(matches!(
        parsed,
        IncomingResponse::Matched {
            result: Value::String(_),
            ..
        }
    ));
}

#[test]
fn non_response_message_type_is_rejected() {
    let request = Value::Array(vec![
        Value::from(0u64),
        Value::from(42u64),
        Value::from("request"),
        Value::Array(vec![]),
    ]);
    assert!(parse_response(request, 42).is_err());

    let unknown = Value::Array(vec![
        Value::from(3u64),
        Value::from(42u64),
        Value::Nil,
        Value::Nil,
    ]);
    assert!(parse_response(unknown, 42).is_err());
}

#[test]
fn malformed_ext_is_an_error() {
    let value = Value::Ext(0, vec![0xc1]);
    assert!(checked_value::ext_or_int(&value).is_err());
}

#[test]
fn non_utf8_string_is_an_error() {
    let mut bytes = &[0xa1, 0xff][..];
    let value = rmpv::decode::read_value(&mut bytes).expect("string value");
    assert!(checked_value::value_to_string(&value).is_err());
}
#[test]
fn happy_path_does_not_probe_mode() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let socket_path = temp_dir.path().join("nvim.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket");
    let (methods_tx, methods_rx) = mpsc::channel();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connection");
        let decoded = rmpv::decode::read_value(&mut stream).expect("request");
        let fields = decoded.as_array().expect("request array");
        methods_tx
            .send(fields[2].as_str().expect("method").to_owned())
            .expect("send");
        let msgid = fields[1].as_u64().expect("msgid");
        let reply = response(msgid, Value::from("ok"));
        rmpv::encode::write_value(&mut stream, &reply).expect("reply");
    });

    let result = nvim_call(&socket_path, "nvim_buf_get_lines", vec![]).expect("call");
    assert_eq!(result.as_str(), Some("ok"));
    assert_eq!(methods_rx.recv().expect("method"), "nvim_buf_get_lines");
    server.join().expect("server");
}
