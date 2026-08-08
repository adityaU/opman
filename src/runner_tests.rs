//! Tests for the runner abstraction and the session->runner registry.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

async fn serve(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    url
}

/// Runners agree on the `/session/status` envelope but not on the word for
/// "still going", and a retry is the same unfinished turn.
#[test]
fn running_status_accepts_every_runners_wording() {
    for kind in ["busy", "retry", "active"] {
        assert!(
            is_running_status(&json!({ "type": kind })),
            "{kind} should read as running"
        );
    }
    assert!(!is_running_status(&json!({ "type": "idle" })));
    assert!(!is_running_status(&json!({})));
    assert!(!is_running_status(&Value::Null));
}

#[test]
fn running_session_ids_keeps_only_the_running_entries() {
    let status = json!({
        "a": { "type": "busy" },
        "b": { "type": "idle" },
        "c": { "type": "retry" },
    });
    let ids = running_session_ids(&status);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains("a") && ids.contains("c"));
    assert!(running_session_ids(&json!([])).is_empty());
}

/// The union across runners is the point: asking only the default runner is
/// what left every other runner's sessions stuck.
#[tokio::test]
async fn status_all_reports_each_runner_separately() {
    let opencode = serve(axum::Router::new().route(
        "/session/status",
        axum::routing::get(|| async { axum::Json(json!({ "s1": { "type": "busy" } })) }),
    ))
    .await;
    let client = reqwest::Client::new();
    let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
    runners.insert(
        RunnerKind::Opencode,
        Arc::new(HttpRunner::new(
            RunnerKind::Opencode,
            opencode,
            client.clone(),
        )),
    );
    // Nothing listens here, so this runner cannot answer.
    runners.insert(
        RunnerKind::ClaudeCode,
        Arc::new(HttpRunner::new(
            RunnerKind::ClaudeCode,
            "http://127.0.0.1:1",
            client,
        )),
    );
    let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);

    let reported: HashMap<String, Option<HashSet<String>>> =
        registry.status_all("/project").await.into_iter().collect();
    assert_eq!(reported.len(), 2);
    assert_eq!(
        reported["opencode"].as_ref().map(HashSet::len),
        Some(1),
        "the reachable runner reports its running session"
    );
    assert!(
        reported["claude-code"].is_none(),
        "an unreachable runner reports nothing, not an empty set"
    );
}

/// A reply is owned by exactly one engine. `ok:false`, transport errors, and
/// missing routes must all read as "not ours" so the registry fan-out reaches
/// the engine that actually raised the request.
#[tokio::test]
async fn http_runner_reply_routes_by_ownership() {
    use axum::routing::post;
    let owner = axum::Router::new().route(
        "/permission/{id}/reply",
        post(|| async { axum::Json(json!({ "ok": true })) }),
    );
    let stranger = axum::Router::new().route(
        "/permission/{id}/reply",
        post(|| async { axum::Json(json!({ "ok": false })) }),
    );
    let client = reqwest::Client::new();
    let owning = HttpRunner::new(RunnerKind::Claude, serve(owner).await, client.clone());
    let other = HttpRunner::new(
        RunnerKind::ClaudeCode,
        serve(stranger).await,
        client.clone(),
    );
    // No such route at all (native opencode shape) → not ours, not an error.
    let no_route = HttpRunner::new(
        RunnerKind::Opencode,
        serve(axum::Router::new()).await,
        client.clone(),
    );
    // Dead port → transport error also reads as not-ours.
    let dead = HttpRunner::new(RunnerKind::Opencode, "http://127.0.0.1:9", client);

    assert!(owning.reply_permission("p1", "once").await.unwrap());
    assert!(!other.reply_permission("p1", "once").await.unwrap());
    assert!(!no_route.reply_permission("p1", "once").await.unwrap());
    assert!(!dead.reply_permission("p1", "once").await.unwrap());
}

#[tokio::test]
async fn http_runner_question_reply_posts_answers() {
    use axum::routing::post;
    let app = axum::Router::new().route(
        "/question/{id}/reply",
        post(|axum::Json(b): axum::Json<Value>| async move {
            let good = b["answers"][0][0] == "A" && b["answers"][1][0] == "B";
            axum::Json(json!({ "ok": good }))
        }),
    );
    let runner = HttpRunner::new(RunnerKind::Claude, serve(app).await, reqwest::Client::new());
    let answers = vec![vec!["A".to_string()], vec!["B".to_string()]];
    assert!(runner.reply_question("q1", &answers).await.unwrap());
}

#[test]
fn parses_runner_names() {
    assert_eq!(
        RunnerKind::parse("claude-code"),
        Some(RunnerKind::ClaudeCode)
    );
    assert_eq!(RunnerKind::parse("nope"), None);
    // Codex is no longer a compile-time runner: it reaches opman as the ACP agent
    // `acp.json` declares, so its label parses only once that config registered it.
    register_acp_runners(["codex".to_string()]);
    assert_eq!(
        RunnerKind::parse("codex"),
        Some(RunnerKind::Acp("codex".to_string()))
    );
}

/// Sends must go to `prompt_async`, not `/message`.
///
/// `POST /session/{id}/message` only responds once the assistant has
/// finished, so awaiting it holds the caller's request open for the whole
/// turn — a long turn then reads as a hang and dies on the client or tunnel
/// timeout. This test stands up a server that would block forever on
/// `/message`: if the endpoint ever regresses, it hangs instead of passing.
#[tokio::test]
async fn send_message_uses_the_non_blocking_prompt_endpoint() {
    use std::sync::Arc as StdArc;
    use tokio::sync::Mutex as AsyncMutex;

    let hits: StdArc<AsyncMutex<Vec<String>>> = StdArc::new(AsyncMutex::new(vec![]));
    let seen = hits.clone();
    let app = axum::Router::new()
        .route(
            "/session/{id}/prompt_async",
            axum::routing::post(
                move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let seen = seen.clone();
                    async move {
                        seen.lock().await.push(format!("prompt_async:{id}"));
                        axum::Json(json!({ "ok": true }))
                    }
                },
            ),
        )
        .route(
            "/session/{id}/message",
            axum::routing::post(|| async {
                // Stand in for the streaming endpoint: never responds.
                std::future::pending::<()>().await;
                axum::Json(json!({}))
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let runner = HttpRunner::new(RunnerKind::Opencode, base, reqwest::Client::new());
    let body = json!({ "parts": [{ "type": "text", "text": "hi" }] });
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        runner.send_message("s1", "/project", body),
    )
    .await
    .expect("send_message hit the blocking /message endpoint")
    .expect("send should succeed");

    assert_eq!(out["ok"], true);
    assert_eq!(hits.lock().await.as_slice(), ["prompt_async:s1"]);
}

/// The OpenCode body rewrite must survive the endpoint change: runner-only
/// controls are stripped and `effort` becomes `variant`.
#[tokio::test]
async fn opencode_send_strips_runner_controls_and_maps_effort() {
    use std::sync::Arc as StdArc;
    use tokio::sync::Mutex as AsyncMutex;

    let seen: StdArc<AsyncMutex<Option<Value>>> = StdArc::new(AsyncMutex::new(None));
    let sink = seen.clone();
    let app = axum::Router::new().route(
        "/session/{id}/prompt_async",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let sink = sink.clone();
            async move {
                *sink.lock().await = Some(body);
                axum::Json(json!({ "ok": true }))
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let runner = HttpRunner::new(RunnerKind::Opencode, base, reqwest::Client::new());
    runner
        .send_message(
            "s1",
            "/project",
            json!({
                "parts": [{ "type": "text", "text": "hi" }],
                "effort": "high",
                "permission": "default",
                "runner": "opencode",
                "agent": "claude",
            }),
        )
        .await
        .expect("send should succeed");

    let body = seen.lock().await.clone().expect("body was forwarded");
    assert_eq!(body["variant"], "high");
    assert!(body.get("effort").is_none());
    assert!(body.get("permission").is_none());
    assert!(body.get("runner").is_none());
    // "claude" is an opman-side label, not an OpenCode agent.
    assert!(body.get("agent").is_none());
}

struct MockRunner {
    kind: RunnerKind,
    prefix: &'static str,
    next: AtomicUsize,
    sessions: RwLock<HashMap<String, Value>>,
}

impl Runner for MockRunner {
    fn kind(&self) -> RunnerKind {
        self.kind.clone()
    }
    fn create_session<'a>(
        &'a self,
        _directory: &'a str,
        title: &'a str,
    ) -> RunnerFuture<'a, RunnerSession> {
        Box::pin(async move {
            let id = format!(
                "{}_{}",
                self.prefix,
                self.next.fetch_add(1, Ordering::Relaxed)
            );
            self.sessions.write().await.insert(id.clone(), json!([]));
            Ok(RunnerSession {
                id,
                title: title.to_string(),
            })
        })
    }
    fn messages<'a>(&'a self, session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            Ok(self
                .sessions
                .read()
                .await
                .get(session_id)
                .cloned()
                .unwrap_or(json!([])))
        })
    }
    fn send_message<'a>(
        &'a self,
        session_id: &'a str,
        _directory: &'a str,
        body: Value,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            self.sessions
                .write()
                .await
                .entry(session_id.to_string())
                .or_insert(json!([]));
            Ok(body)
        })
    }
    fn execute_command<'a>(
        &'a self,
        session_id: &'a str,
        _directory: &'a str,
        command: SlashCommand<'a>,
    ) -> RunnerFuture<'a, Value> {
        Box::pin(async move {
            Ok(json!({
                "runner": self.kind.display_name(),
                "session": session_id,
                "command": command.name,
                "arguments": command.arguments,
            }))
        })
    }
    fn abort<'a>(&'a self, _session_id: &'a str, _directory: &'a str) -> RunnerFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn switching_runner_creates_a_handoff_session_with_summary(
) -> Result<(), Box<dyn std::error::Error>> {
    let old = Arc::new(MockRunner {
        kind: RunnerKind::Opencode,
        prefix: "old",
        next: AtomicUsize::new(1),
        sessions: RwLock::new(HashMap::new()),
    });
    old.sessions.write().await.insert(
        "logical".into(),
        json!([
            { "info": { "role": "user" }, "parts": [{ "text": "Fix the parser" }] },
            { "info": { "role": "assistant" }, "parts": [{ "text": "I found the parser" }] }
        ]),
    );
    let new = Arc::new(MockRunner {
        kind: RunnerKind::ClaudeCode,
        prefix: "new",
        next: AtomicUsize::new(1),
        sessions: RwLock::new(HashMap::new()),
    });
    let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
    runners.insert(RunnerKind::Opencode, old.clone());
    runners.insert(RunnerKind::ClaudeCode, new.clone());
    let registry = RunnerRegistry::new(RunnerKind::Opencode, runners);
    let outcome = registry
        .send_message(
            "logical",
            "/project",
            Some(RunnerKind::ClaudeCode),
            json!({
                "parts": [{ "type": "text", "text": "Now add a regression test" }]
            }),
        )
        .await?;
    assert!(outcome.switched);
    assert_eq!(outcome.runner, RunnerKind::ClaudeCode);
    assert!(outcome.session_id.starts_with("new_"));
    let handoff = outcome.response["parts"][0]["text"]
        .as_str()
        .ok_or("handoff response did not contain text")?;
    assert!(handoff.contains("Fix the parser"));
    Ok(())
}

fn two_runner_registry() -> (Arc<MockRunner>, Arc<MockRunner>, RunnerRegistry) {
    let default = Arc::new(MockRunner {
        kind: RunnerKind::ClaudeCode,
        prefix: "cc",
        next: AtomicUsize::new(1),
        sessions: RwLock::new(HashMap::new()),
    });
    let other = Arc::new(MockRunner {
        kind: RunnerKind::Claude,
        prefix: "cp",
        next: AtomicUsize::new(1),
        sessions: RwLock::new(HashMap::new()),
    });
    let mut runners: HashMap<RunnerKind, Arc<dyn Runner>> = HashMap::new();
    runners.insert(RunnerKind::ClaudeCode, default.clone());
    runners.insert(RunnerKind::Claude, other.clone());
    let registry = RunnerRegistry::new(RunnerKind::ClaudeCode, runners);
    (default, other, registry)
}

/// A slash command is executed by the runner holding the transcript, never by the
/// default engine. This is the bug the routing exists to prevent: the command list is
/// already answered per runner, so running a name the session's own runner advertised
/// against a different engine fails with "session not found".
#[tokio::test]
async fn slash_commands_run_on_the_session_owner_not_the_default_runner(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_default, _other, registry) = two_runner_registry();
    let created = registry
        .create_session(RunnerKind::Claude, "/project", "chat")
        .await?;

    let result = registry
        .execute_command(
            &created.id,
            "/project",
            SlashCommand::new("compact", "keep the plan", None),
        )
        .await?;

    // The fixture's default is ClaudeCode; the session belongs to Claude.
    assert_eq!(result["runner"], *RunnerKind::Claude.display_name());
    assert_eq!(result["session"], created.id);
    assert_eq!(result["command"], "compact");
    assert_eq!(result["arguments"], "keep the plan");
    Ok(())
}

/// The follow-up turn of an ordinary conversation must land in the same
/// session. A send that names no runner is not a switch request, so it stays
/// on whatever runner the session is bound to.
#[tokio::test]
async fn sends_without_a_requested_runner_stay_on_the_bound_runner(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_default, other, registry) = two_runner_registry();
    let created = registry
        .create_session(RunnerKind::Claude, "/project", "chat")
        .await?;

    for turn in ["first", "second"] {
        let outcome = registry
            .send_message(
                &created.id,
                "/project",
                None,
                json!({ "parts": [{ "type": "text", "text": turn }] }),
            )
            .await?;
        assert!(!outcome.switched, "turn {turn} forked the session");
        assert_eq!(outcome.session_id, created.id);
        assert_eq!(outcome.runner, RunnerKind::Claude);
    }
    // The default runner never saw the conversation.
    assert!(other.sessions.read().await.contains_key(&created.id));
    Ok(())
}

/// Naming the runner the session already uses is not a switch either — the
/// UI may legitimately restate it on a new session's first turn.
#[tokio::test]
async fn restating_the_bound_runner_is_not_a_switch() -> Result<(), Box<dyn std::error::Error>> {
    let (_default, _other, registry) = two_runner_registry();
    let created = registry
        .create_session(RunnerKind::Claude, "/project", "chat")
        .await?;
    let outcome = registry
        .send_message(
            &created.id,
            "/project",
            Some(RunnerKind::Claude),
            json!({ "parts": [{ "type": "text", "text": "hi" }] }),
        )
        .await?;
    assert!(!outcome.switched);
    assert_eq!(outcome.session_id, created.id);
    Ok(())
}

/// Bindings are in-memory, so a session opman learned about from somewhere
/// else (its runner label, the session poller) has none. Without adopting
/// it, the blind default-runner fallback turns its next turn into a handoff.
#[tokio::test]
async fn ensure_binding_adopts_a_session_without_handing_it_off(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_default, other, registry) = two_runner_registry();
    other
        .sessions
        .write()
        .await
        .insert("orphan".into(), json!([]));

    registry
        .ensure_binding("orphan", RunnerKind::Claude, "/project")
        .await;
    let outcome = registry
        .send_message(
            "orphan",
            "/project",
            None,
            json!({ "parts": [{ "type": "text", "text": "resume" }] }),
        )
        .await?;
    assert!(!outcome.switched);
    assert_eq!(outcome.runner, RunnerKind::Claude);
    assert_eq!(outcome.session_id, "orphan");

    // Adoption never overrides an established binding.
    registry
        .ensure_binding("orphan", RunnerKind::ClaudeCode, "/project")
        .await;
    assert_eq!(registry.runner_for("orphan").await, RunnerKind::Claude);
    Ok(())
}

/// Without adoption the same send is read as "switch to the default runner"
/// and forks the conversation — the regression this pair of tests guards.
#[tokio::test]
async fn an_unadopted_session_still_hands_off_when_a_runner_is_named(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_default, other, registry) = two_runner_registry();
    other
        .sessions
        .write()
        .await
        .insert("orphan".into(), json!([]));
    let outcome = registry
        .send_message(
            "orphan",
            "/project",
            Some(RunnerKind::Claude),
            json!({ "parts": [{ "type": "text", "text": "resume" }] }),
        )
        .await?;
    assert!(outcome.switched);
    assert_ne!(outcome.session_id, "orphan");
    Ok(())
}
