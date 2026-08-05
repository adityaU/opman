//! Generated coverage tests for `routes.rs` — pure helpers + router endpoints.
use super::*;
use crate::claude_engine::registry::SessionEntry;
use axum::http::StatusCode;

fn engine() -> Engine {
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

/// Local mirror of `crate::web::test_support::send_json` for the engine router.
async fn send(
    router: Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let mut builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => {
            builder = builder.header("content-type", "application/json");
            builder.body(Body::empty()).unwrap()
        }
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

// ── pure helpers ────────────────────────────────────────────────────

#[test]
fn dir_header_reads_and_defaults() {
    let mut h = HeaderMap::new();
    assert_eq!(dir_header(&h), "");
    h.insert("x-opencode-directory", "/proj".parse().unwrap());
    assert_eq!(dir_header(&h), "/proj");
    // Non-UTF8 header value → empty (to_str fails).
    h.insert(
        "x-opencode-directory",
        axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
    );
    assert_eq!(dir_header(&h), "");
}

#[test]
fn session_obj_shape() {
    let entry = SessionEntry {
        id: "ses_1".into(),
        title: "Hi".into(),
        directory: "/d".into(),
        parent_id: "ses_p".into(),
        created: 111,
        updated: 222,
        ..Default::default()
    };
    let v = session_obj(&entry);
    assert_eq!(v["id"], "ses_1");
    assert_eq!(v["title"], "Hi");
    assert_eq!(v["parentID"], "ses_p");
    assert_eq!(v["directory"], "/d");
    assert_eq!(v["projectID"], "claude");
    assert_eq!(v["time"]["created"], 111);
    assert_eq!(v["time"]["updated"], 222);
}

#[test]
fn extract_text_variants() {
    // Parts with text joined.
    let b = json!({ "parts": [
        { "type": "text", "text": "one" },
        { "type": "file", "text": "ignored" },
        { "type": "text", "text": "two" },
    ]});
    assert_eq!(extract_text(&b), "one\ntwo");
    // Parts present but no text → fall back to top-level text.
    let b = json!({ "parts": [ { "type": "file" } ], "text": "fallback" });
    assert_eq!(extract_text(&b), "fallback");
    // No parts, prompt fallback.
    let b = json!({ "prompt": "p" });
    assert_eq!(extract_text(&b), "p");
    // Nothing at all.
    assert_eq!(extract_text(&json!({})), "");
    // Part without explicit type defaults to text.
    let b = json!({ "parts": [ { "text": "def" } ] });
    assert_eq!(extract_text(&b), "def");
}

#[test]
fn with_attachments_appends_or_passes_through() {
    assert_eq!(with_attachments("hi".into(), &[]), "hi");
    let out = with_attachments("hi".into(), &["/a/b.png".to_string()]);
    assert!(out.starts_with("hi\n\n[Attached file(s)"));
    assert!(out.contains("- /a/b.png"));
}

#[test]
fn save_attachments_writes_decodes_and_skips() {
    let sid = format!("test-sess-{}", rand::random::<u64>());
    let body = json!({ "parts": [
        // text part → ignored
        { "type": "text", "text": "hello" },
        // valid data-url file ("hello")
        { "type": "file", "filename": "note.txt", "url": "data:text/plain;base64,aGVsbG8=" },
        // image via source.url, no filename → synthesized name
        { "type": "image", "source": { "url": "data:image/png;base64,aGk=" } },
        // non-data url → skipped
        { "type": "file", "url": "https://example.com/x.txt" },
        // invalid base64 → skipped
        { "type": "file", "url": "data:text/plain;base64,!!!!" },
        // path-traversal filename is sanitized to basename
        { "type": "file", "filename": "../../evil.txt", "url": "data:text/plain;base64,aGVsbG8=" },
    ]});
    let saved = save_attachments(&body, &sid);
    assert_eq!(saved.len(), 3);
    for p in &saved {
        assert!(std::path::Path::new(p).exists());
        // No traversal escaped the upload dir.
        assert!(p.contains(&sid));
        assert!(!p.contains(".."));
    }
    // Cleanup.
    let dir = std::env::temp_dir().join("opman-uploads").join(&sid);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_attachments_no_parts_is_empty() {
    assert!(save_attachments(&json!({}), "s").is_empty());
    assert!(save_attachments(&json!({ "parts": "notarray" }), "s").is_empty());
}

#[test]
fn command_and_agent_descriptions() {
    assert_eq!(
        command_description("compact"),
        "Compact the conversation to save context"
    );
    assert_eq!(
        command_description("verify"),
        "Verify a change by running the app"
    );
    assert_eq!(command_description("totally-unknown"), "");
    assert_eq!(
        agent_description("claude"),
        "Default agent for general tasks"
    );
    assert_eq!(
        agent_description("Explore"),
        "Fast read-only codebase search and exploration"
    );
    assert_eq!(agent_description("nope"), "");
}

#[test]
fn hook_allow_and_deny_shapes() {
    let a = hook_allow();
    assert_eq!(a["hookSpecificOutput"]["permissionDecision"], "allow");
    let d = hook_deny("because");
    assert_eq!(d["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(
        d["hookSpecificOutput"]["permissionDecisionReason"],
        "because"
    );
}

#[test]
fn rand_request_id_format() {
    let id = rand_request_id();
    assert!(id.starts_with("perm_"));
    assert_eq!(id.len(), "perm_".len() + 32);
    assert_ne!(rand_request_id(), rand_request_id());
}

#[test]
fn permission_patterns_extraction() {
    let inp = json!({
        "file_path": "/f", "path": "/p", "notebook_path": "/n", "command": "ls -la"
    });
    let out = permission_patterns(&inp);
    assert_eq!(out, vec!["/f", "/p", "/n", "ls -la"]);
    assert!(permission_patterns(&json!({})).is_empty());
}

#[test]
fn build_questions_maps_and_empty() {
    assert_eq!(build_questions(&json!({}), "s"), json!([]));
    let inp = json!({ "questions": [
        { "question": "Q1?", "header": "H", "multiSelect": true,
          "options": [ { "label": "A", "description": "da" }, { "label": "B" } ] },
        { "question": "Q2?" },
    ]});
    let out = build_questions(&inp, "s");
    let arr = out.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["question"], "Q1?");
    assert_eq!(arr[0]["header"], "H");
    assert_eq!(arr[0]["multiple"], true);
    assert_eq!(arr[0]["custom"], true);
    assert_eq!(arr[0]["options"][0]["label"], "A");
    assert_eq!(arr[0]["options"][0]["description"], "da");
    assert_eq!(arr[0]["options"][1]["description"], "");
    assert_eq!(arr[1]["multiple"], false);
    assert!(arr[1]["options"].as_array().unwrap().is_empty());
}

#[test]
fn format_answers_builds_lines() {
    let inp = json!({ "questions": [ { "question": "Fruit?" }, { "question": "Color?" } ] });
    let ans = vec![
        vec!["Apple".to_string()],
        vec!["Red".to_string(), "Blue".to_string()],
    ];
    let out = format_answers(&inp, &ans);
    assert!(out.starts_with("[USER ANSWER]"));
    assert!(out.contains("Fruit? → Apple"));
    assert!(out.contains("Color? → Red, Blue"));
    // Missing question text falls back.
    let out2 = format_answers(&json!({}), &[vec!["X".to_string()]]);
    assert!(out2.contains("(question) → X"));
}

#[test]
fn handle_control_command_permission_modes() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    assert!(handle_control_command(
        &e,
        &s.id,
        "/permission-mode acceptEdits"
    ));
    assert_eq!(e.effective_mode(&s.id), "acceptEdits");
    assert!(handle_control_command(&e, &s.id, "/perm-mode plan"));
    assert_eq!(e.effective_mode(&s.id), "plan");
    assert!(handle_control_command(&e, &s.id, "/perm bypassPermissions"));
    assert_eq!(e.effective_mode(&s.id), "bypassPermissions");
    // Unknown mode → consumed, mode unchanged, error toast emitted.
    assert!(handle_control_command(&e, &s.id, "/permission-mode bogus"));
    assert_eq!(e.effective_mode(&s.id), "bypassPermissions");
    // Unknown mode with a missing session → still consumed, no panic.
    assert!(handle_control_command(&e, "no-such", "/perm bogus"));
    // A plain prompt is not a control command.
    assert!(!handle_control_command(&e, &s.id, "hello world"));
}

#[test]
fn handle_control_command_agent_toast_path() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    // Seed a real agent list so "explore" resolves and emits a toast.
    e.set_cached_init(
        "/d",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["Explore".into()],
        },
    );
    assert!(handle_control_command(&e, &s.id, "/agent explore"));
    assert_eq!(
        e.get_session(&s.id).unwrap().agent.as_deref(),
        Some("Explore")
    );
    // Whitespace-only name after /agent → no-op but consumed.
    assert!(handle_control_command(&e, &s.id, "/agent    "));
}

#[test]
fn dispatch_turn_empty_and_queue_and_command() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    // Empty text → nothing queued/spawned.
    dispatch_turn(e.clone(), s.id.clone(), "   ".into());
    assert!(e.pending_list(&s.id).is_empty());
    // Control command consumed, nothing queued.
    dispatch_turn(e.clone(), s.id.clone(), "/perm plan".into());
    assert!(e.pending_list(&s.id).is_empty());
    // Occupied session → the prompt queues.
    e.set_busy(&s.id, true);
    dispatch_turn(e.clone(), s.id.clone(), "hi there".into());
    assert_eq!(e.pending_list(&s.id), vec!["hi there".to_string()]);
}

// ── router endpoints ────────────────────────────────────────────────

#[tokio::test]
async fn info_and_health() {
    let r = router(engine());
    let (st, body) = send(r.clone(), "GET", "/health", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, b"ok");
    let (st, body) = send(r, "GET", "/info", None).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_of(&body);
    assert!(v.get("version").is_some());
    assert_eq!(v["directory"], "");
}

#[tokio::test]
async fn provider_returns_default_models() {
    let (st, body) = send(router(engine()), "GET", "/provider", None).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_of(&body);
    assert_eq!(v["all"][0]["id"], "anthropic");
    assert_eq!(v["connected"][0], "anthropic");
    // Both engines share `models::pick_default`: the first sonnet-or-fable in the
    // catalog wins, which for the fallback list is Fable 5.
    assert_eq!(v["default"]["anthropic"], "claude-fable-5");
    assert!(v["all"][0]["models"]["claude-opus-5"].is_object());
}

#[tokio::test]
async fn provider_uses_cached_models_when_present() {
    let e = engine();
    e.set_cached_models(vec![
        claude_cli::ModelInfo {
            id: "claude-fable-1".into(),
            display_name: "Fable".into(),
            context_window: 42,
            max_output: 7,
        },
        claude_cli::ModelInfo {
            id: "claude-opus-x".into(),
            display_name: "Opus X".into(),
            context_window: 9,
            max_output: 3,
        },
    ]);
    let (st, body) = send(router(e), "GET", "/provider", None).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_of(&body);
    // Prefers fable/sonnet for the default.
    assert_eq!(v["default"]["anthropic"], "claude-fable-1");
    let m = &v["all"][0]["models"]["claude-fable-1"];
    assert_eq!(m["providerID"], "anthropic");
    assert_eq!(m["name"], "Fable");
    assert_eq!(m["limit"]["context"], 42);
    assert_eq!(m["limit"]["output"], 7);
}

#[tokio::test]
async fn session_crud_lifecycle() {
    let e = engine();
    let r = router(e.clone());
    // Create (empty dir header so list_sessions skips the claude subprocess).
    let (st, body) = send(
        r.clone(),
        "POST",
        "/session",
        Some(json!({ "title": "My session" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let created = json_of(&body);
    let id = created["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("ses_"));
    assert_eq!(created["title"], "My session");

    // Get existing + missing.
    let (_, body) = send(r.clone(), "GET", &format!("/session/{id}"), None).await;
    assert_eq!(json_of(&body)["title"], "My session");
    let (_, body) = send(r.clone(), "GET", "/session/ses_missing", None).await;
    assert_eq!(json_of(&body)["id"], "ses_missing");

    // Rename.
    let (_, body) = send(
        r.clone(),
        "PATCH",
        &format!("/session/{id}"),
        Some(json!({ "title": "Renamed" })),
    )
    .await;
    assert_eq!(json_of(&body)["title"], "Renamed");
    // Rename with no title in body → returns current entry.
    let (_, body) = send(
        r.clone(),
        "PATCH",
        &format!("/session/{id}"),
        Some(json!({})),
    )
    .await;
    assert_eq!(json_of(&body)["title"], "Renamed");
    // Rename a missing session → `{ id }`.
    let (_, body) = send(
        r.clone(),
        "PATCH",
        "/session/ses_x",
        Some(json!({ "title": "z" })),
    )
    .await;
    assert_eq!(json_of(&body)["id"], "ses_x");

    // List for empty dir.
    let (st, body) = send(r.clone(), "GET", "/session", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(json_of(&body)
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == id.as_str()));

    // Delete (no short_id → no stop subprocess).
    let (_, body) = send(r.clone(), "DELETE", &format!("/session/{id}"), None).await;
    assert_eq!(json_of(&body)["ok"], true);
    assert!(e.get_session(&id).is_none());
}

#[tokio::test]
async fn session_status_reports_busy_only() {
    let e = engine();
    let r = router(e.clone());
    let a = e.create_session("", "", "a");
    let b = e.create_session("", "", "b");
    e.set_busy(&a.id, true);
    let (st, body) = send(r, "GET", "/session/status", None).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_of(&body);
    assert_eq!(v[&a.id]["type"], "busy");
    assert!(v.get(&b.id).is_none()); // idle absent
}

#[tokio::test]
async fn messages_and_todos_empty_without_transcript() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("", "", "t");
    let (st, body) = send(
        r.clone(),
        "GET",
        &format!("/session/{}/message", s.id),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(json_of(&body).as_array().unwrap().is_empty());
    // Unknown session id (not a subagent transcript) → empty array.
    let (_, body) = send(r.clone(), "GET", "/session/nope/message", None).await;
    assert!(json_of(&body).as_array().unwrap().is_empty());
    // Todos with no claude session → empty.
    let (st, body) = send(r, "GET", &format!("/session/{}/todo", s.id), None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(json_of(&body).as_array().unwrap().is_empty());
}

#[tokio::test]
async fn queue_endpoints() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.enqueue_prompt(&s.id, "one".into());
    e.enqueue_prompt(&s.id, "two".into());
    e.enqueue_prompt(&s.id, "three".into());

    let (st, body) = send(r.clone(), "GET", &format!("/session/{}/queue", s.id), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_of(&body)["pending"].as_array().unwrap().len(), 3);

    // Remove middle item by index.
    let (_, body) = send(
        r.clone(),
        "DELETE",
        &format!("/session/{}/queue/1", s.id),
        None,
    )
    .await;
    let v = json_of(&body);
    assert_eq!(v["ok"], true);
    assert_eq!(v["pending"], json!(["one", "three"]));

    // Out-of-range index → ok false.
    let (_, body) = send(
        r.clone(),
        "DELETE",
        &format!("/session/{}/queue/9", s.id),
        None,
    )
    .await;
    assert_eq!(json_of(&body)["ok"], false);

    // Clear all.
    let (_, body) = send(r, "DELETE", &format!("/session/{}/queue", s.id), None).await;
    assert_eq!(json_of(&body)["pending"], json!([]));
    assert!(e.pending_list(&s.id).is_empty());
}

#[tokio::test]
async fn send_message_sets_model_agent_and_queues_when_busy() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.set_cached_init(
        "/d",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
        },
    );
    e.set_busy(&s.id, true); // occupied → the turn queues instead of spawning claude.
    let body = json!({
        "model": { "providerID": "anthropic", "modelID": "claude-opus-4-8" },
        "agent": "plan",
        "parts": [ { "type": "text", "text": "do it" } ],
    });
    let (st, resp) = send(r, "POST", &format!("/session/{}/message", s.id), Some(body)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_of(&resp)["ok"], true);
    let entry = e.get_session(&s.id).unwrap();
    assert_eq!(entry.model.as_deref(), Some("claude-opus-4-8"));
    assert_eq!(entry.agent.as_deref(), Some("Plan"));
    assert_eq!(e.pending_list(&s.id), vec!["do it".to_string()]);
}

/// `prompt_async` is the route the runner actually sends to, so it has to honour
/// the same model/agent/effort controls as `/message` — otherwise a selected
/// model is silently dropped on every send.
#[tokio::test]
async fn prompt_async_applies_model_agent_and_effort() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.set_cached_init(
        "/d",
        claude_cli::InitInfo {
            commands: vec![],
            agents: vec!["Plan".into()],
        },
    );
    e.set_busy(&s.id, true); // occupied → the turn queues instead of spawning claude.
    let body = json!({
        "model": { "providerID": "anthropic", "modelID": "claude-opus-4-8" },
        "agent": "plan",
        "effort": "high",
        "parts": [ { "type": "text", "text": "do it" } ],
    });
    let (st, resp) = send(
        r,
        "POST",
        &format!("/session/{}/prompt_async", s.id),
        Some(body),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_of(&resp)["ok"], true);
    let entry = e.get_session(&s.id).unwrap();
    assert_eq!(entry.model.as_deref(), Some("claude-opus-4-8"));
    assert_eq!(entry.agent.as_deref(), Some("Plan"));
    assert_eq!(e.pending_list(&s.id), vec!["do it".to_string()]);
}

#[tokio::test]
async fn prompt_async_and_session_command_queue_when_busy() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.set_busy(&s.id, true);
    let (st, _) = send(
        r.clone(),
        "POST",
        &format!("/session/{}/prompt_async", s.id),
        Some(json!({ "prompt": "async hi" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    // session_command builds "/cmd args".
    let (st, _) = send(
        r,
        "POST",
        &format!("/session/{}/command", s.id),
        Some(json!({ "command": "compact", "arguments": "now" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        e.pending_list(&s.id),
        vec!["async hi".to_string(), "/compact now".to_string()]
    );
}

#[tokio::test]
async fn session_command_without_args() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.set_busy(&s.id, true);
    send(
        r,
        "POST",
        &format!("/session/{}/command", s.id),
        Some(json!({ "command": "usage" })),
    )
    .await;
    assert_eq!(e.pending_list(&s.id), vec!["/usage".to_string()]);
}

#[tokio::test]
async fn abort_clears_queue_and_marks_idle() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    e.set_busy(&s.id, true);
    e.enqueue_prompt(&s.id, "later".into());
    let (st, body) = send(r, "POST", &format!("/session/{}/abort", s.id), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_of(&body)["ok"], true);
    assert!(e.pending_list(&s.id).is_empty());
    assert!(!e.get_session(&s.id).unwrap().busy);
}

// NOTE: the `/reap` route calls `reaper::reap_once`, which shells out to the real
// `claude` binary and would stop live background agents on this host — deliberately
// NOT exercised here to avoid killing real sessions. Its 2-line handler is uncovered.
#[tokio::test]
async fn noop_and_select_endpoints() {
    let e = engine();
    let r = router(e.clone());
    let s = e.create_session("/d", "", "t");
    for path in [
        format!("/session/{}/revert", s.id),
        format!("/session/{}/unrevert", s.id),
    ] {
        let (st, body) = send(r.clone(), "POST", &path, Some(json!({}))).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(json_of(&body)["ok"], true);
    }
    let (_, body) = send(
        r.clone(),
        "POST",
        &format!("/session/{}/share", s.id),
        Some(json!({})),
    )
    .await;
    assert_eq!(json_of(&body), json!({}));
    let (st, body) = send(r, "POST", "/tui/select-session", Some(json!({ "x": 1 }))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_of(&body)["ok"], true);
}

#[tokio::test]
async fn command_and_agent_lists() {
    let e = engine();
    let r = router(e.clone());
    // Empty dir header → empty lists (no subprocess).
    let (_, body) = send(r.clone(), "GET", "/command", None).await;
    assert!(json_of(&body).as_array().unwrap().is_empty());
    let (_, body) = send(r.clone(), "GET", "/agent", None).await;
    assert!(json_of(&body).as_array().unwrap().is_empty());

    // With a cached init for a directory, no subprocess is spawned.
    e.set_cached_init(
        "/proj",
        claude_cli::InitInfo {
            commands: vec!["compact".into(), "my-custom".into()],
            agents: vec!["claude".into(), "Explore".into()],
        },
    );

    let req = |uri: &'static str| {
        let router = r.clone();
        async move {
            use axum::body::Body;
            use axum::http::Request;
            use tower::ServiceExt;
            let req = Request::builder()
                .method("GET")
                .uri(uri)
                .header("x-opencode-directory", "/proj")
                .body(Body::empty())
                .unwrap();
            let resp = router.oneshot(req).await.unwrap();
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec();
            serde_json::from_slice::<Value>(&bytes).unwrap()
        }
    };

    let cmds = req("/command").await;
    let arr = cmds.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr
        .iter()
        .any(|c| c["name"] == "compact" && c["description"] != ""));
    // Unknown command has no description field.
    assert!(arr
        .iter()
        .any(|c| c["name"] == "my-custom" && c.get("description").is_none()));

    let agents = req("/agent").await;
    let arr = agents.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr
        .iter()
        .any(|a| a["name"] == "claude" && a["mode"] == "primary" && a["native"] == true));
    assert!(arr
        .iter()
        .any(|a| a["name"] == "Explore" && a["mode"] == "all"));
}
