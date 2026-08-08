use super::*;

use crate::acp_engine::config::{AgentConfig, ClientCaps};

/// An engine with one session, bound to the ACP session id the requests below name.
fn engine(terminal: bool) -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    let agent = AgentConfig {
        client_caps: ClientCaps {
            terminal,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = Arc::new(AcpEngine::new("test".to_string(), agent, None, registry));
    let session = engine.create_session("/tmp", "", "terminals");
    engine.bind_acp_session(&session.id, "acp-1");
    engine
}

fn request(extra: Value) -> Value {
    let mut params = json!({ "sessionId": "acp-1" });
    if let (Some(params), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
        params.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    params
}

async fn spawn(engine: &Arc<AcpEngine>, command: &str, args: Value) -> String {
    let created = create(
        engine,
        &request(json!({ "command": command, "args": args })),
    )
    .await
    .expect("terminal/create should succeed");
    created["terminalId"]
        .as_str()
        .expect("a terminal id")
        .to_string()
}

/// The capability is a promise, and refusing plainly is how an agent learns to use its own
/// tools instead. What must never happen is opman advertising it and then hanging.
#[tokio::test]
async fn terminals_are_refused_when_the_capability_is_off() {
    let engine = engine(false);
    let refused = create(&engine, &request(json!({ "command": "true" }))).await;
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("terminal/create"), "{message}");
}

#[tokio::test]
async fn a_command_runs_and_reports_its_output_and_exit_code() {
    let engine = engine(true);
    let id = spawn(&engine, "echo", json!(["hello"])).await;

    let exit = wait_for_exit(&engine, &request(json!({ "terminalId": id })))
        .await
        .expect("wait_for_exit should succeed");
    assert_eq!(exit["exitCode"], 0);

    let out = output(&engine, &request(json!({ "terminalId": id }))).expect("output");
    assert_eq!(out["output"], "hello\n");
    assert_eq!(out["truncated"], false);
    assert_eq!(out["exitStatus"]["exitCode"], 0);
}

/// stderr is part of what a terminal shows, so it lands in the same buffer as stdout.
#[tokio::test]
async fn stderr_is_captured_too() {
    let engine = engine(true);
    let id = spawn(&engine, "sh", json!(["-c", "echo oops >&2"])).await;
    let _ = wait_for_exit(&engine, &request(json!({ "terminalId": id }))).await;
    let out = output(&engine, &request(json!({ "terminalId": id }))).expect("output");
    assert_eq!(out["output"], "oops\n");
}

#[tokio::test]
async fn a_failing_command_reports_its_code() {
    let engine = engine(true);
    let id = spawn(&engine, "sh", json!(["-c", "exit 3"])).await;
    let exit = wait_for_exit(&engine, &request(json!({ "terminalId": id })))
        .await
        .expect("wait_for_exit");
    assert_eq!(exit["exitCode"], 3);
}

/// The case a shared `Mutex<Child>` would deadlock on: killing something already being
/// waited on. The waiter has to come back, and come back saying it was signalled.
#[tokio::test]
async fn killing_a_waited_on_command_releases_the_waiter() {
    let engine = engine(true);
    let id = spawn(&engine, "sleep", json!(["30"])).await;

    let waiting = {
        let engine = engine.clone();
        let id = id.clone();
        tokio::spawn(
            async move { wait_for_exit(&engine, &request(json!({ "terminalId": id }))).await },
        )
    };
    // Let the waiter reach its await before the kill, so this tests the racy order.
    tokio::task::yield_now().await;
    kill(&engine, &request(json!({ "terminalId": id }))).expect("kill");

    let exit = tokio::time::timeout(std::time::Duration::from_secs(10), waiting)
        .await
        .expect("the waiter should not hang")
        .expect("the waiting task should not panic")
        .expect("wait_for_exit");
    assert_eq!(exit["signal"], "SIGKILL");
}

/// A killed terminal keeps its output: the agent asked for it to stop, not to be forgotten.
#[tokio::test]
async fn a_killed_terminal_still_reads_back() {
    let engine = engine(true);
    let id = spawn(&engine, "sh", json!(["-c", "echo before; sleep 30"])).await;
    // Wait for the write to land rather than for a fixed delay.
    while output(&engine, &request(json!({ "terminalId": id })))
        .map(|out| out["output"] == "")
        .unwrap_or(true)
    {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    kill(&engine, &request(json!({ "terminalId": id }))).expect("kill");
    let out = output(&engine, &request(json!({ "terminalId": id }))).expect("output");
    assert_eq!(out["output"], "before\n");
}

/// Releasing is the agent saying it will not read this one again.
#[tokio::test]
async fn a_released_terminal_is_gone() {
    let engine = engine(true);
    let id = spawn(&engine, "echo", json!(["x"])).await;
    release(&engine, &request(json!({ "terminalId": id }))).expect("release");

    let missing = output(&engine, &request(json!({ "terminalId": id })));
    let message = missing.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("already have been released"), "{message}");
}

/// Releasing twice is not an error: the agent may release on its own cleanup path and again
/// when the session ends.
#[tokio::test]
async fn releasing_twice_is_harmless() {
    let engine = engine(true);
    let id = spawn(&engine, "echo", json!(["x"])).await;
    assert!(release(&engine, &request(json!({ "terminalId": id.clone() }))).is_ok());
    assert!(release(&engine, &request(json!({ "terminalId": id }))).is_ok());
}

/// Deleting a session takes its commands with it — a terminal nobody can read from is a
/// process running for no one.
#[tokio::test]
async fn deleting_a_session_releases_its_terminals() {
    let engine = engine(true);
    let id = spawn(&engine, "sleep", json!(["30"])).await;
    let session = engine
        .list_for_dir("/tmp")
        .first()
        .map(|s| s.id.clone())
        .expect("a session");

    engine.delete_session(&session).await;
    assert!(engine.terminals.get(&id).is_none());
}

/// A request naming a terminal that never existed is an error, not a silent empty read.
#[tokio::test]
async fn an_unknown_terminal_is_an_error() {
    let engine = engine(true);
    assert!(output(&engine, &request(json!({ "terminalId": "nope" }))).is_err());
    assert!(kill(&engine, &request(json!({ "terminalId": "nope" }))).is_err());
}
