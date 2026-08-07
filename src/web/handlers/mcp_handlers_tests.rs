//! Reading, toggling and removing MCP servers.
//!
//! Writing one is covered next door in `mcp_upsert_tests`. Every test here redirects
//! `mcp.json` and the token store at a temp directory, so nothing reads or writes the
//! developer's real configuration or credentials.

use super::*;

use crate::web::test_support::{test_server_state, ConfigRedirect};

fn open() -> AuthUser {
    AuthUser {
        subject: String::new(),
    }
}

fn stdio_server() -> ServerConfig {
    ServerConfig {
        command: "npx".into(),
        args: vec!["-y".into(), "pkg".into()],
        ..ServerConfig::default()
    }
}

/// Toggling a built-in writes a patch, not a full definition — so opman's own launch
/// command is never copied into user config where it would then go stale.
#[tokio::test]
async fn disabling_a_builtin_writes_only_the_toggle() {
    let redirect = ConfigRedirect::new();
    let state = test_server_state();
    let _saved = set_enabled(
        open(),
        State(state.clone()),
        Path("time".to_string()),
        Json(SetEnabled { enabled: false }),
    )
    .await
    .expect("saved");

    let entry = &redirect.document().servers["time"];
    assert!(!entry.enabled);
    assert!(
        entry.command.is_empty(),
        "no launch command should be written"
    );
    assert!(!entry.defines_transport());
}

#[tokio::test]
async fn deleting_an_unknown_server_is_a_404() {
    let _redirect = ConfigRedirect::new();
    let state = test_server_state();
    let result = delete_server(open(), State(state), Path("ghost".to_string())).await;
    assert!(matches!(result, Err(StatusCode::NOT_FOUND)));
}

#[tokio::test]
async fn delete_removes_the_entry() {
    let redirect = ConfigRedirect::new();
    redirect.declare("temp", stdio_server());
    let state = test_server_state();
    let _deleted = delete_server(open(), State(state), Path("temp".to_string()))
        .await
        .expect("deleted");
    assert!(!redirect.document().servers.contains_key("temp"));
}

#[tokio::test]
async fn listing_includes_builtins_with_no_entry_of_their_own() {
    let _redirect = ConfigRedirect::new();
    let mut state = test_server_state();
    state.mcp = crate::mcp_registry::RegistryHandle::load(crate::mcp_registry::BuiltinFlags::ALL);
    let Json(list) = list_servers(open(), State(state)).await.expect("ok");
    let names: Vec<_> = list.iter().map(|s| s.name.as_str()).collect();
    // Toggleable without the user first having to hand-write a stub entry.
    assert!(names.contains(&"time"));
    assert!(
        list.iter()
            .find(|s| s.name == "time")
            .expect("time")
            .builtin
    );
}

#[tokio::test]
async fn listing_reports_names_but_never_values_for_env_and_headers() {
    let redirect = ConfigRedirect::new();
    redirect.declare(
        "secretive",
        ServerConfig {
            url: "https://x/mcp".into(),
            auth: "static".into(),
            env: [("TOKEN".to_string(), "hunter2".to_string())]
                .into_iter()
                .collect(),
            headers: [("Authorization".to_string(), "Bearer hunter2".to_string())]
                .into_iter()
                .collect(),
            ..ServerConfig::default()
        },
    );
    let state = test_server_state();
    let Json(list) = list_servers(open(), State(state)).await.expect("ok");
    let entry = list
        .iter()
        .find(|s| s.name == "secretive")
        .expect("declared server");

    assert_eq!(entry.env_names, vec!["TOKEN".to_string()]);
    assert_eq!(entry.header_names, vec!["Authorization".to_string()]);
    let json = serde_json::to_string(&entry).expect("serialize");
    assert!(
        !json.contains("hunter2"),
        "a credential must not reach the browser: {json}"
    );
}

#[test]
fn server_names_are_validated() {
    assert!(validate("linear").is_ok());
    assert!(validate("my-server_1.0").is_ok());
    for bad in ["", "   ", "../evil", "a/b", "has space", &"x".repeat(65)] {
        assert!(validate(bad).is_err(), "{bad:?} must be rejected");
    }
}

#[test]
fn a_remote_with_a_credential_is_reported_as_proxied() {
    let mut entry = ServerConfig {
        url: "https://x/mcp".into(),
        auth: "oauth".into(),
        ..ServerConfig::default()
    };
    assert!(view("x", &entry, false, false).proxied);
    entry.auth = "none".into();
    assert!(!view("x", &entry, false, false).proxied);
}

#[test]
fn login_status_is_reported_independently_of_whether_a_server_is_proxied() {
    let entry = ServerConfig {
        url: "https://x/mcp".into(),
        auth: "oauth".into(),
        ..ServerConfig::default()
    };
    assert!(!view("x", &entry, false, false).authenticated);
    assert!(view("x", &entry, false, true).authenticated);
}
