//! Shared setup for the live suites: finding a running opman, and speaking its socket.
//!
//! These talk to the manager exactly as a bridge child does, so the tests that use them
//! cover the whole path a tool call takes — wire format, dispatch, registry, a real runner,
//! a real model. Nothing is faked.
//!
//! Every live test is `#[ignore]`d *and* gated on [`enabled`], because an accidental
//! tree-wide `--ignored` run would otherwise spend tokens on somebody's account.

use std::path::PathBuf;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// The two runner/model pairs under test: opman's ACP claude agent, and opencode driving
/// an OpenAI model. Different engines, different transports, different session id shapes.
pub(super) const CLAUDE_MODEL: &str = "haiku";
pub(super) const LUNA_MODEL: &str = "gpt-5.6-luna";
pub(super) const LUNA_PROVIDER: &str = "openai";

/// Whatever the reply is, it has to be traceable to the prompt that caused it.
pub(super) const CLAUDE_SENTINEL: &str = "HAIKU-OK";
pub(super) const LUNA_SENTINEL: &str = "LUNA-OK";

/// Skip unless the operator asked for real turns.
pub(super) fn enabled() -> bool {
    let on = std::env::var("OPMAN_LIVE_AGENT_TESTS").is_ok_and(|value| value != "0");
    if !on {
        eprintln!("skipping: set OPMAN_LIVE_AGENT_TESTS=1 to run live agent-manager tests");
    }
    on
}

pub(super) fn directory() -> String {
    std::env::var("OPMAN_LIVE_DIR").unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "/".to_string())
    })
}

/// The socket of the opman these tests should talk to.
///
/// Named by the environment inside a runner child; discovered from `/tmp` when the test is
/// run from a shell. Sockets are PID-scoped and a dead opman leaves its file behind, so the
/// newest is the live one.
fn socket() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(super::SOCKET_ENV) {
        return Some(PathBuf::from(path));
    }
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(std::env::temp_dir())
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("opman-agent-manager-") && name.ends_with(".sock")
                })
        })
        .filter_map(|path| Some((path.metadata().ok()?.modified().ok()?, path)))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().next().map(|(_, path)| path)
}

/// One request over the socket, in the bridge's own framing.
pub(super) async fn call(request: Value) -> Value {
    let path = socket().expect("no running opman: no agent-manager socket found");
    let mut stream = UnixStream::connect(&path)
        .await
        .unwrap_or_else(|error| panic!("connect {}: {error}", path.display()));
    stream
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("write");
    stream.shutdown().await.expect("shutdown");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .await
        .expect("read");
    let response: Value = serde_json::from_str(line.trim()).expect("a json envelope");
    assert_eq!(
        response["ok"], true,
        "manager refused {request}: {}",
        response["error"]
    );
    response["data"].clone()
}

pub(super) fn start(runner: &str, model: &str, provider: Option<&str>, message: &str) -> Value {
    json!({
        "op": "start", "directory": directory(), "runner": runner,
        "model": model, "provider": provider, "effort": "low",
        "title": "live agent-manager test", "message": message,
    })
}

/// Start an agent, wait for it, and insist it said what it was asked to say.
pub(super) async fn round_trip(
    runner: &str,
    model: &str,
    provider: Option<&str>,
    sentinel: &str,
) -> String {
    let started = call(start(
        runner,
        model,
        provider,
        &format!("Reply with exactly one word: {sentinel}"),
    ))
    .await;
    let session = started["session_id"]
        .as_str()
        .expect("a started session has an id")
        .to_string();
    assert_eq!(started["runner"], runner);

    let waited = call(json!({
        "op": "wait", "directory": directory(), "target": session, "timeout": 180,
    }))
    .await;

    assert_eq!(
        waited["timed_out"], false,
        "{runner} never finished: {waited}"
    );
    let reply = waited["reply"].as_str().unwrap_or_default();
    assert!(
        reply.contains(sentinel),
        "{runner}/{model} replied {reply:?}, expected {sentinel}",
    );
    session
}
