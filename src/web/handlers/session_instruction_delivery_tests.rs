//! Opening instructions reach each runner through the real session handler.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use axum::http::StatusCode;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::runner::{Runner, RunnerFuture, RunnerKind, RunnerRegistry, RunnerSession};
use crate::web::test_support::{send_json, test_router, test_server_state_with_projects};
use crate::web::types::{CreatePersonalMemoryRequest, MemoryScope};

struct RecordingRunner {
    kind: RunnerKind,
    received: Arc<Mutex<HashMap<String, Value>>>,
    history: Value,
    failures_left: Arc<AtomicUsize>,
}

impl Runner for RecordingRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            Ok(RunnerSession {
                id: "created".to_string(),
                title: title.to_string(),
            })
        })
    }

    fn messages<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, Value> {
        let history = self.history.clone();
        Box::pin(async move { Ok(history) })
    }

    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        _: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            if self
                .failures_left
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(anyhow!("first send rejected"));
            }
            self.received
                .lock()
                .await
                .insert(session_id.to_string(), body.clone());
            Ok(body)
        })
    }

    fn abort<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn opening_instructions_reach_every_registered_runner() {
    opman_backend_contracts::register_acp_runners(["codex".to_string(), "antigravity".to_string()]);
    let temp = tempfile::tempdir().expect("temporary project");
    let directory = temp.path().to_string_lossy().into_owned();
    let mut state = test_server_state_with_projects(vec![(
        "instructions-test".to_string(),
        temp.path().to_path_buf(),
    )]);
    state
        .web_state
        .create_personal_memory(CreatePersonalMemoryRequest {
            label: "Style".to_string(),
            content: "Keep changes small".to_string(),
            scope: MemoryScope::Global,
            project_index: None,
            session_id: None,
        })
        .await;

    let kinds = [
        RunnerKind::Opencode,
        RunnerKind::Claude,
        RunnerKind::ClaudeCode,
        RunnerKind::Acp("codex".to_string()),
        RunnerKind::Acp("antigravity".to_string()),
    ];
    let received = Arc::new(Mutex::new(HashMap::new()));
    let runners = kinds
        .iter()
        .cloned()
        .map(|kind| {
            let runner = RecordingRunner {
                kind: kind.clone(),
                received: Arc::clone(&received),
                history: if kind == RunnerKind::Acp("antigravity".to_string()) {
                    json!([
                        { "info": { "role": "system" }, "parts": [{ "type": "text", "text": "setup" }] },
                        { "info": { "role": "assistant" }, "parts": [{ "type": "text", "text": "ready" }] }
                    ])
                } else {
                    json!([])
                },
                failures_left: Arc::new(AtomicUsize::new(0)),
            };
            (kind, Arc::new(runner) as Arc<dyn Runner>)
        })
        .collect();
    state.runner_registry = Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners));

    for (index, kind) in kinds.iter().enumerate() {
        let session_id = format!("session-{index}");
        state
            .web_state
            .set_session_runner(&session_id, kind.display_name().as_ref())
            .await;
        state
            .runner_registry
            .ensure_binding(&session_id, kind.clone(), &directory)
            .await;
        let (status, _) = send_json(
            test_router(state.clone()),
            "POST",
            &format!("/api/session/{session_id}/message"),
            Some(if kind == &RunnerKind::Acp("antigravity".to_string()) {
                json!({ "parts": [{ "type": "file", "url": "data:image/png;base64,AA" }] })
            } else {
                json!({ "parts": [{ "type": "text", "text": "Build the feature" }] })
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{}", kind.display_name());
    }

    let received = received.lock().await;
    assert_eq!(received.len(), kinds.len());
    for index in 0..kinds.len() {
        let session_id = format!("session-{index}");
        let text = received
            .get(&session_id)
            .and_then(|body| body.get("parts"))
            .and_then(Value::as_array)
            .and_then(|parts| parts.iter().find_map(|part| part.get("text")))
            .and_then(Value::as_str);
        assert!(
            text.is_some_and(
                |text| text.starts_with("[Session instructions]\n- Style: Keep changes small")
            ),
            "opening instructions missing for {}: {text:?}",
            kinds[index].display_name()
        );
        if index != 4 {
            assert!(text.is_some_and(|text| text.ends_with("Build the feature")));
        } else {
            assert_eq!(received[&session_id]["parts"][1]["type"], "file");
        }
    }
}

#[tokio::test]
async fn rejected_first_send_releases_instruction_claim_for_retry() {
    opman_backend_contracts::register_acp_runners(["antigravity".to_string()]);
    let temp = tempfile::tempdir().expect("temporary project");
    let directory = temp.path().to_string_lossy().into_owned();
    let mut state = test_server_state_with_projects(vec![(
        "retry-test".to_string(),
        temp.path().to_path_buf(),
    )]);
    state
        .web_state
        .create_personal_memory(CreatePersonalMemoryRequest {
            label: "Style".to_string(),
            content: "Keep changes small".to_string(),
            scope: MemoryScope::Global,
            project_index: None,
            session_id: None,
        })
        .await;

    let kind = RunnerKind::Acp("antigravity".to_string());
    let received = Arc::new(Mutex::new(HashMap::new()));
    let runner = RecordingRunner {
        kind: kind.clone(),
        received: Arc::clone(&received),
        history: json!([
            { "info": { "role": "system" } },
            { "info": { "role": "assistant" } }
        ]),
        failures_left: Arc::new(AtomicUsize::new(1)),
    };
    state.runner_registry = Arc::new(RunnerRegistry::new(
        RunnerKind::Opencode,
        [(kind.clone(), Arc::new(runner) as Arc<dyn Runner>)]
            .into_iter()
            .collect(),
    ));
    let session_id = "retry-session";
    state
        .web_state
        .set_session_runner(session_id, kind.display_name().as_ref())
        .await;
    state
        .runner_registry
        .ensure_binding(session_id, kind, &directory)
        .await;

    for expected in [StatusCode::INTERNAL_SERVER_ERROR, StatusCode::OK] {
        let (status, _) = send_json(
            test_router(state.clone()),
            "POST",
            &format!("/api/session/{session_id}/message"),
            Some(json!({ "parts": [{ "type": "text", "text": "Try again" }] })),
        )
        .await;
        assert_eq!(status, expected);
    }
    let captured = received.lock().await;
    let text = captured[session_id]["parts"][0]["text"].as_str();
    assert!(text.is_some_and(|text| text.starts_with("[Session instructions]")));
    assert!(text.is_some_and(|text| text.ends_with("Try again")));
}
