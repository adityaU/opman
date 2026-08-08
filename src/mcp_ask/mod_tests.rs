use super::*;

use std::time::Duration;

/// A context with no loopback: `tools/call` answers from the "opman is not running" branch
/// without touching the network, which is what makes the read loop testable in isolation.
fn offline() -> Context {
    Context {
        loopback: None,
        session: Some("ses-1".to_string()),
        directory: "/repo".to_string(),
    }
}

type Shared = Arc<Mutex<Vec<u8>>>;

async fn drive(context: Context, input: &str) -> Vec<Value> {
    let out: Shared = Arc::new(Mutex::new(Vec::new()));
    run_ask_bridge(context, input.as_bytes(), out.clone())
        .await
        .expect("the loop ends cleanly at EOF");
    let written = out.lock().await.clone();
    String::from_utf8(written)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is one json message"))
        .collect()
}

#[tokio::test]
async fn the_handshake_and_catalogue_answer_in_order() {
    let messages = drive(
        offline(),
        concat!(
            r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#,
            "\n",
        ),
    )
    .await;

    assert_eq!(messages.len(), 2, "the notification is not answered");
    assert_eq!(messages[0]["id"], 1);
    assert_eq!(messages[0]["result"]["serverInfo"]["name"], "opman-ask");
    assert_eq!(messages[1]["result"]["tools"][0]["name"], tools::TOOL_NAME);
}

#[tokio::test]
async fn malformed_and_unknown_input_is_answered_rather_than_dropped() {
    let messages = drive(
        offline(),
        concat!(
            "\n",
            "{not json\n",
            r#"{"jsonrpc":"2.0","method":"nope","id":9}"#,
            "\n",
        ),
    )
    .await;

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["error"]["code"], -32700);
    assert!(messages[0]["id"].is_null());
    assert_eq!(messages[1]["error"]["code"], -32601);
    assert_eq!(messages[1]["id"], 9);
}

#[tokio::test]
async fn a_call_answers_with_tool_text() {
    let call = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 3,
        "params": {
            "name": tools::TOOL_NAME,
            "arguments": { "questions": [{
                "question": "Which database?",
                "header": "DB",
                "options": [{ "label": "Postgres" }, { "label": "SQLite" }],
            }] },
        },
    });
    let messages = drive(offline(), &format!("{call}\n")).await;

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["id"], 3);
    let text = messages[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    assert!(text.contains("not running"), "got: {text}");
}

#[tokio::test]
async fn a_malformed_call_complains_in_the_result_not_as_an_error() {
    let call = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 4,
        "params": { "name": tools::TOOL_NAME, "arguments": {} },
    });
    let messages = drive(offline(), &format!("{call}\n")).await;

    // A schema complaint the model can act on beats a JSON-RPC error it cannot see.
    assert!(messages[0].get("error").is_none());
    let text = messages[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    assert!(text.contains("non-empty array"), "got: {text}");
}

#[tokio::test]
async fn an_unknown_tool_name_is_named_back() {
    let call = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 5,
        "params": { "name": "something_else", "arguments": {} },
    });
    let messages = drive(offline(), &format!("{call}\n")).await;
    assert_eq!(
        messages[0]["result"]["content"][0]["text"],
        "Unknown tool: something_else"
    );
}

#[tokio::test]
async fn eof_writes_nothing() {
    assert!(drive(offline(), "").await.is_empty());
}

// ── cancellation ────────────────────────────────────────────────────

#[test]
fn in_flight_tracks_ids_as_written() {
    // JSON-RPC ids may be numbers or strings, and `1` is not `"1"`.
    assert_ne!(key(&json!(1)), key(&json!("1")));
    assert_eq!(key(&json!("abc")), key(&json!("abc")));
}

#[tokio::test]
async fn cancelling_a_call_ends_the_wait_without_answering_it() {
    // A never-resolving call, so only the cancellation can end it.
    let handle = tokio::spawn(std::future::pending::<()>());
    let mut in_flight = InFlight::default();
    in_flight.track(&json!(7), handle.abort_handle());

    in_flight.cancel(&json!(7));
    assert!(handle.await.is_err(), "the call was aborted");
    // Cancelling twice, or cancelling something that never existed, is a no-op.
    in_flight.cancel(&json!(7));
    in_flight.cancel(&json!("never-seen"));
}

#[tokio::test]
async fn finished_calls_are_pruned_but_waiting_ones_are_kept() {
    let done = tokio::spawn(async {});
    let waiting = tokio::spawn(std::future::pending::<()>());
    let mut in_flight = InFlight::default();
    in_flight.track(&json!(1), done.abort_handle());
    in_flight.track(&json!(2), waiting.abort_handle());
    done.await.expect("completes");

    in_flight.prune();
    assert_eq!(in_flight.0.len(), 1);
    assert!(in_flight.0.contains_key(&key(&json!(2))));
    waiting.abort();
}

/// A `notifications/cancelled` for a call that is still waiting must not be answered
/// afterwards: the client has moved on, and a late result would be attributed to nothing.
#[tokio::test]
async fn a_cancelled_call_writes_no_result() {
    // A listener that completes the handshake and then says nothing, so the call is
    // genuinely blocked on an answer the way a real question is.
    let silent = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = silent.local_addr().expect("addr").port();
    let context = Context {
        loopback: Some(crate::loopback::Loopback {
            url: format!("http://127.0.0.1:{port}"),
            token: "t".to_string(),
            client: reqwest::Client::new(),
        }),
        session: Some("ses-1".to_string()),
        directory: "/repo".to_string(),
    };

    let out: Shared = Arc::new(Mutex::new(Vec::new()));
    let (mut writer, reader) = tokio::io::duplex(1024);
    let loop_out = out.clone();
    let running = tokio::spawn(async move { run_ask_bridge(context, reader, loop_out).await });

    let call = json!({
        "jsonrpc": "2.0", "method": "tools/call", "id": 11,
        "params": { "name": tools::TOOL_NAME, "arguments": { "questions": [{
            "question": "Which database?", "header": "DB",
            "options": [{ "label": "Postgres" }, { "label": "SQLite" }],
        }] } },
    });
    let cancel = json!({
        "jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": { "requestId": 11, "reason": "turn aborted" },
    });
    use tokio::io::AsyncWriteExt;
    writer
        .write_all(format!("{call}\n{cancel}\n").as_bytes())
        .await
        .expect("write");
    writer.shutdown().await.expect("eof");

    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("the loop ends")
        .expect("no join error")
        .expect("clean exit");
    assert!(
        out.lock().await.is_empty(),
        "a cancelled call is owed no result"
    );
}
