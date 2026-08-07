//! Writing one server into `mcp.json`.
//!
//! The behaviour worth pinning down is what a *partial* body does, because the settings
//! page is never shown an `env` or `headers` value and so can never send one back.

use super::*;

use crate::web::test_support::{test_server_state, ConfigRedirect};

fn open() -> AuthUser {
    AuthUser {
        subject: String::new(),
    }
}

/// A full definition, as the add form submits one.
fn stdio_body() -> UpsertServer {
    UpsertServer {
        command: Some("npx".into()),
        args: Some(vec!["-y".into(), "pkg".into()]),
        ..UpsertServer::default()
    }
}

async fn put(state: &ServerState, name: &str, body: UpsertServer) -> Result<(), StatusCode> {
    upsert_server(
        open(),
        State(state.clone()),
        Path(name.to_string()),
        Json(body),
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn upsert_writes_the_entry_and_reloads_the_registry() {
    let redirect = ConfigRedirect::new();
    let state = test_server_state();
    put(&state, "browser", stdio_body()).await.expect("saved");

    assert_eq!(redirect.document().servers["browser"].command, "npx");
    // The swap is what makes the edit apply without restarting opman.
    assert!(state.mcp.current().get("browser").is_some());
}

#[tokio::test]
async fn a_new_entry_with_no_transport_is_rejected() {
    let _redirect = ConfigRedirect::new();
    let state = test_server_state();
    let result = put(&state, "bad", UpsertServer::default()).await;
    assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
}

/// A built-in needs no command of its own: an entry naming one is a patch on opman's
/// definition, so scoping it to two runners must not demand a transport as well.
#[tokio::test]
async fn a_builtin_can_be_patched_without_declaring_a_transport() {
    let redirect = ConfigRedirect::new();
    let state = test_server_state();
    put(
        &state,
        "time",
        UpsertServer {
            runners: Some(vec!["codex".into()]),
            ..UpsertServer::default()
        },
    )
    .await
    .expect("saved");

    let entry = &redirect.document().servers["time"];
    assert!(entry.command.is_empty());
    assert_eq!(entry.runners.len(), 1);
}

#[tokio::test]
async fn an_unknown_runner_name_is_rejected_rather_than_silently_dropped() {
    let _redirect = ConfigRedirect::new();
    let state = test_server_state();
    let result = put(
        &state,
        "x",
        UpsertServer {
            runners: Some(vec!["not-a-runner".into()]),
            ..stdio_body()
        },
    )
    .await;
    assert!(matches!(result, Err(StatusCode::UNPROCESSABLE_ENTITY)));
}

#[tokio::test]
async fn an_absent_field_is_left_alone_rather_than_cleared() {
    let redirect = ConfigRedirect::new();
    redirect.declare(
        "linear",
        ServerConfig {
            url: "https://mcp.linear.app/sse".into(),
            auth: "oauth".into(),
            args: vec!["--keep".into()],
            env: [("TOKEN".to_string(), "hunter2".to_string())]
                .into_iter()
                .collect(),
            headers: [("X-Keep".to_string(), "yes".to_string())]
                .into_iter()
                .collect(),
            timeout_secs: Some(90),
            ..ServerConfig::default()
        },
    );
    let state = test_server_state();

    // What the edit form sends when the user only changed the transport kind. It has never
    // seen the env or header values, so it cannot resend them — and must not erase them.
    put(
        &state,
        "linear",
        UpsertServer {
            r#type: Some("http".into()),
            ..UpsertServer::default()
        },
    )
    .await
    .expect("saved");

    let entry = &redirect.document().servers["linear"];
    assert_eq!(entry.r#type, "http");
    assert_eq!(entry.url, "https://mcp.linear.app/sse");
    assert_eq!(entry.auth, "oauth");
    assert_eq!(entry.args, vec!["--keep".to_string()]);
    assert_eq!(entry.env.get("TOKEN").map(String::as_str), Some("hunter2"));
    assert_eq!(entry.headers.get("X-Keep").map(String::as_str), Some("yes"));
    assert_eq!(entry.timeout_secs, Some(90));
}

#[tokio::test]
async fn an_empty_list_clears_where_an_absent_one_preserves() {
    let redirect = ConfigRedirect::new();
    redirect.declare(
        "verbose",
        ServerConfig {
            command: "npx".into(),
            args: vec!["-y".into(), "pkg".into()],
            timeout_secs: Some(30),
            ..ServerConfig::default()
        },
    );
    let state = test_server_state();
    put(
        &state,
        "verbose",
        UpsertServer {
            args: Some(Vec::new()),
            timeout_secs: Some(None),
            ..UpsertServer::default()
        },
    )
    .await
    .expect("saved");

    let entry = &redirect.document().servers["verbose"];
    assert!(entry.args.is_empty(), "an explicit empty list clears");
    assert_eq!(entry.timeout_secs, None, "an explicit null clears");
}

#[tokio::test]
async fn secrets_are_edited_by_name() {
    let redirect = ConfigRedirect::new();
    redirect.declare(
        "keyed",
        ServerConfig {
            command: "npx".into(),
            env: [
                ("STALE".to_string(), "old".to_string()),
                ("KEEP".to_string(), "kept".to_string()),
            ]
            .into_iter()
            .collect(),
            headers: [("X-Gone".to_string(), "bye".to_string())]
                .into_iter()
                .collect(),
            ..ServerConfig::default()
        },
    );
    let state = test_server_state();
    put(
        &state,
        "keyed",
        UpsertServer {
            env_set: [("STALE".to_string(), "fresh".to_string())]
                .into_iter()
                .collect(),
            env_remove: vec!["ABSENT".into()],
            headers_remove: vec!["X-Gone".into()],
            ..UpsertServer::default()
        },
    )
    .await
    .expect("saved");

    let entry = &redirect.document().servers["keyed"];
    assert_eq!(entry.env.get("STALE").map(String::as_str), Some("fresh"));
    assert_eq!(entry.env.get("KEEP").map(String::as_str), Some("kept"));
    assert!(entry.headers.is_empty());
}

/// A key in both `set` and `remove` ends up gone. Arbitrary either way, so it is fixed
/// here rather than left to map-iteration order.
#[test]
fn removal_wins_over_setting_the_same_key() {
    let mut entry = ServerConfig {
        command: "npx".into(),
        ..ServerConfig::default()
    };
    apply(
        &mut entry,
        UpsertServer {
            env_set: [("A".to_string(), "1".to_string())].into_iter().collect(),
            env_remove: vec!["A".into()],
            ..UpsertServer::default()
        },
        false,
    )
    .expect("applied");
    assert!(entry.env.is_empty());
}
