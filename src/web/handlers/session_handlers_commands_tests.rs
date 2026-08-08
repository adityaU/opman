//! `GET /api/commands` routes to the runner that will execute the command.
//!
//! The endpoint used to ask the default engine no matter which runner served the session,
//! so a claude or ACP conversation was offered opencode's commands. These tests stand up two
//! mock engines with disjoint command lists and check that each runner answers for itself.

use crate::web::test_support::{
    scope_base_url, send_json, start_mock_upstream, test_router, test_server_state,
};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::json;

fn isolate_env() {
    use std::sync::OnceLock;
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let _env_guard = crate::claude_engine::claude_cli::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", d.path());
        std::env::set_var("XDG_STATE_HOME", d.path());
        d
    });
}

async fn state_with_project() -> (ServerState, tempfile::TempDir) {
    isolate_env();
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().expect("utf-8 path"), None)
        .await
        .expect("add project");
    (state, tmp)
}

/// A registry holding both runners, each pointed at its own mock engine.
async fn state_with_two_runners(
    opencode_base: &str,
    claude_base: &str,
) -> (ServerState, tempfile::TempDir) {
    let (mut state, tmp) = state_with_project().await;
    let client = reqwest::Client::new();
    let mut runners: std::collections::HashMap<
        crate::runner::RunnerKind,
        std::sync::Arc<dyn crate::runner::Runner>,
    > = std::collections::HashMap::new();
    for (kind, base) in [
        (crate::runner::RunnerKind::Opencode, opencode_base),
        (crate::runner::RunnerKind::Claude, claude_base),
    ] {
        runners.insert(
            kind.clone(),
            std::sync::Arc::new(crate::runner::HttpRunner::new(kind, base, client.clone())),
        );
    }
    state.runner_registry = std::sync::Arc::new(crate::runner::RunnerRegistry::new(
        crate::runner::RunnerKind::Opencode,
        runners,
    ));
    (state, tmp)
}

fn command_names(body: &[u8]) -> Vec<String> {
    let parsed: serde_json::Value = serde_json::from_slice(body).expect("json array");
    parsed
        .as_array()
        .expect("array of commands")
        .iter()
        .filter_map(|c| c.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect()
}

/// Two engines, two vocabularies. `?runner=` decides which one answers.
async fn two_engines() -> (String, String) {
    let opencode = start_mock_upstream(axum::Router::new().route(
        "/command",
        get(|| async { axum::Json(json!([{ "name": "share" }, { "name": "undo" }])) }),
    ))
    .await;
    let claude = start_mock_upstream(axum::Router::new().route(
        "/command",
        get(|| async { axum::Json(json!([{ "name": "security-review" }])) }),
    ))
    .await;
    (opencode, claude)
}

#[tokio::test]
async fn a_named_runner_answers_with_its_own_commands() {
    let (opencode, claude) = two_engines().await;
    let (state, _tmp) = state_with_two_runners(&opencode, &claude).await;

    let (status, body) = scope_base_url(
        // The default engine is still opencode; naming a runner must override it.
        opencode.clone(),
        send_json(
            test_router(state),
            "GET",
            "/api/commands?runner=claude",
            None,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(command_names(&body), vec!["security-review".to_string()]);
}

#[tokio::test]
async fn the_other_runner_still_gets_its_own() {
    let (opencode, claude) = two_engines().await;
    let (state, _tmp) = state_with_two_runners(&opencode, &claude).await;

    let (status, body) = scope_base_url(
        opencode.clone(),
        send_json(
            test_router(state),
            "GET",
            "/api/commands?runner=opencode",
            None,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        command_names(&body),
        vec!["share".to_string(), "undo".to_string()]
    );
}

#[tokio::test]
async fn an_unnamed_runner_falls_back_to_the_default_engine() {
    let (opencode, claude) = two_engines().await;
    let (state, _tmp) = state_with_two_runners(&opencode, &claude).await;

    let (status, body) = scope_base_url(
        opencode.clone(),
        send_json(test_router(state), "GET", "/api/commands", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        command_names(&body),
        vec!["share".to_string(), "undo".to_string()]
    );
}

#[tokio::test]
async fn an_unknown_runner_is_rejected_rather_than_silently_answered() {
    let (opencode, claude) = two_engines().await;
    let (state, _tmp) = state_with_two_runners(&opencode, &claude).await;

    let (status, _body) = scope_base_url(
        opencode.clone(),
        send_json(
            test_router(state),
            "GET",
            "/api/commands?runner=not-a-runner",
            None,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}
