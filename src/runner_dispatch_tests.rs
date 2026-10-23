//! A handoff stops its source before reading its final transcript or starting a target.
use super::*;
use tokio::sync::Mutex;

struct Probe {
    kind: RunnerKind,
    events: Arc<Mutex<Vec<&'static str>>>,
    abort_fails: bool,
    pause: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
}

impl Runner for Probe {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }

    fn create_session<'a>(&'a self, _: &'a str, title: &'a str) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            self.events.lock().await.push("create");
            Ok(RunnerSession {
                id: "handoff_target".into(),
                title: title.into(),
            })
        })
    }

    fn messages<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.events.lock().await.push("history");
            Ok(json!([
                {"info": {"role": "user"}, "parts": [{"type": "text", "text": "Fix the build"}]},
                {"info": {"role": "assistant"}, "parts": [{"type": "text", "text": "Found a broken import"}]}
            ]))
        })
    }

    fn send_message<'a>(&'a self, _: &'a str, _: &'a str, body: Value) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.events.lock().await.push("send");
            Ok(body)
        })
    }

    fn abort<'a>(&'a self, _: &'a str, _: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async move {
            self.events.lock().await.push("abort");
            if let Some((entered, resume)) = &self.pause {
                entered.notify_one();
                resume.notified().await;
            }
            if self.abort_fails {
                bail!("source cannot be stopped");
            }
            Ok(())
        })
    }
}

fn registry(abort_fails: bool) -> (RunnerRegistry, Arc<Mutex<Vec<&'static str>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let runners = [RunnerKind::Opencode, RunnerKind::ClaudeCode]
        .into_iter()
        .map(|kind| {
            let probe = Probe {
                kind: kind.clone(),
                events: Arc::clone(&events),
                abort_fails,
                pause: None,
            };
            (kind, Arc::new(probe) as Arc<dyn Runner>)
        })
        .collect();
    (RunnerRegistry::new(RunnerKind::Opencode, runners), events)
}

#[tokio::test]
async fn handoff_stops_source_then_sends_final_history_and_retry() -> Result<()> {
    let (registry, events) = registry(false);
    let outcome = registry
        .send_message(
            "handoff_source",
            "/project",
            Some(RunnerKind::ClaudeCode),
            json!({"parts": [{"type": "text", "text": "Continue the failed request"}],
            "model": {"providerID": "anthropic", "modelID": "opus"}, "permission": "default"}),
        )
        .await?;
    assert_eq!(*events.lock().await, ["abort", "history", "create", "send"]);
    assert!(outcome.switched);
    assert_eq!(outcome.session_id, "handoff_target");
    let prompt = extract_text(&outcome.response);
    assert!(prompt.contains("Fix the build"));
    assert!(prompt.contains("Found a broken import"));
    assert!(prompt.ends_with("Continue the failed request"));
    assert_eq!(outcome.response["model"]["modelID"], "opus");
    assert_eq!(
        registry.runner_for("handoff_target").await,
        RunnerKind::ClaudeCode
    );
    events.lock().await.clear();
    let next = registry
        .send_message(
            "handoff_source",
            "/project",
            None,
            json!({"parts": [{"type": "text", "text": "follow up"}]}),
        )
        .await?;
    assert_eq!(next.session_id, "handoff_target");
    assert_eq!(next.runner, RunnerKind::ClaudeCode);
    assert_eq!(*events.lock().await, ["send"]);
    Ok(())
}

#[tokio::test]
async fn failed_stop_never_starts_a_second_runner() {
    let (registry, events) = registry(true);
    let result = registry
        .send_message(
            "stop_failure",
            "/project",
            Some(RunnerKind::ClaudeCode),
            json!({"parts": [{"type": "text", "text": "continue"}]}),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(*events.lock().await, ["abort"]);
}

#[tokio::test]
async fn ordinary_send_does_not_abort_its_runner() -> Result<()> {
    let (registry, events) = registry(false);
    let outcome = registry
        .send_message(
            "same_runner",
            "/project",
            Some(RunnerKind::Opencode),
            json!({"parts": [{"type": "text", "text": "follow up"}]}),
        )
        .await?;
    assert!(!outcome.switched);
    assert_eq!(*events.lock().await, ["send"]);
    Ok(())
}

#[tokio::test]
async fn concurrent_switches_stop_and_create_only_once() -> Result<()> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(tokio::sync::Notify::new());
    let resume = Arc::new(tokio::sync::Notify::new());
    let source = Arc::new(Probe {
        kind: RunnerKind::Opencode,
        events: Arc::clone(&events),
        abort_fails: false,
        pause: Some((Arc::clone(&entered), Arc::clone(&resume))),
    });
    let target = Arc::new(Probe {
        kind: RunnerKind::ClaudeCode,
        events: Arc::clone(&events),
        abort_fails: false,
        pause: None,
    });
    let runners = HashMap::from([
        (RunnerKind::Opencode, source as Arc<dyn Runner>),
        (RunnerKind::ClaudeCode, target as Arc<dyn Runner>),
    ]);
    let registry = Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners));
    let first_registry = Arc::clone(&registry);
    let first = tokio::spawn(async move {
        first_registry
            .send_message(
                "race_source",
                "/project",
                Some(RunnerKind::ClaudeCode),
                json!({"parts": [{"type": "text", "text": "continue"}]}),
            )
            .await
    });
    entered.notified().await;
    let second = registry
        .send_message(
            "race_source",
            "/project",
            Some(RunnerKind::ClaudeCode),
            json!({"parts": [{"type": "text", "text": "continue"}]}),
        )
        .await;
    assert!(second.is_err());
    resume.notify_one();
    assert!(first.await??.switched);
    assert_eq!(*events.lock().await, ["abort", "history", "create", "send"]);
    Ok(())
}
