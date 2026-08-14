use super::*;
use serde_json::json;

#[test]
fn login_request_deserialize() {
    let r: LoginRequest =
        serde_json::from_value(json!({"username": "u", "password": "p"})).unwrap();
    assert_eq!(r.username, "u");
    assert_eq!(r.password, "p");
}

#[test]
fn login_response_serialize() {
    let v = serde_json::to_value(LoginResponse {
        token: "tok".into(),
    })
    .unwrap();
    assert_eq!(v["token"], "tok");
}

#[test]
fn project_management_requests() {
    let sw: SwitchProjectRequest = serde_json::from_value(json!({"index": 3})).unwrap();
    assert_eq!(sw.index, 3);

    let sel: SelectSessionRequest =
        serde_json::from_value(json!({"project_idx": 1, "session_id": "s"})).unwrap();
    assert_eq!(sel.project_idx, 1);
    assert_eq!(sel.session_id, "s");

    let ns: NewSessionRequest = serde_json::from_value(json!({"project_idx": 2})).unwrap();
    assert_eq!(ns.project_idx, 2);

    let rm: RemoveProjectRequest = serde_json::from_value(json!({"index": 5})).unwrap();
    assert_eq!(rm.index, 5);
}

#[test]
fn new_session_response_serialize() {
    let v = serde_json::to_value(NewSessionResponse {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v["session_id"], "s");
}

#[test]
fn add_project_request_full_and_default() {
    let full: AddProjectRequest =
        serde_json::from_value(json!({"path": "/p", "name": "N"})).unwrap();
    assert_eq!(full.path, "/p");
    assert_eq!(full.name.as_deref(), Some("N"));

    let minimal: AddProjectRequest = serde_json::from_value(json!({"path": "/p"})).unwrap();
    assert!(minimal.name.is_none());
}

#[test]
fn add_project_response_serialize() {
    let v = serde_json::to_value(AddProjectResponse {
        index: 7,
        name: "n".into(),
    })
    .unwrap();
    assert_eq!(v["index"], 7);
    assert_eq!(v["name"], "n");
}

#[test]
fn browse_dirs_request_default_path() {
    let empty: BrowseDirsRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty.path, "");
    let set: BrowseDirsRequest = serde_json::from_value(json!({"path": "/home"})).unwrap();
    assert_eq!(set.path, "/home");
}

#[test]
fn dir_entry_and_browse_dirs_response_serialize() {
    let resp = BrowseDirsResponse {
        path: "/a".into(),
        parent: "/".into(),
        entries: vec![DirEntry {
            name: "sub".into(),
            path: "/a/sub".into(),
            is_project: true,
        }],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["path"], "/a");
    assert_eq!(v["parent"], "/");
    assert_eq!(v["entries"][0]["name"], "sub");
    assert_eq!(v["entries"][0]["is_project"], true);
}

#[test]
fn home_dir_response_serialize() {
    let v = serde_json::to_value(HomeDirResponse {
        path: "/home/u".into(),
    })
    .unwrap();
    assert_eq!(v["path"], "/home/u");
}

#[test]
fn panel_requests() {
    let t: TogglePanelRequest = serde_json::from_value(json!({"panel": "sidebar"})).unwrap();
    assert_eq!(t.panel, "sidebar");
    let f: FocusPanelRequest = serde_json::from_value(json!({"panel": "editor"})).unwrap();
    assert_eq!(f.panel, "editor");
}

#[test]
fn spawn_pty_request_full_and_optional() {
    let full: SpawnPtyRequest = serde_json::from_value(json!({
        "kind": "opencode",
        "id": "p1",
        "rows": 40,
        "cols": 120,
        "project": "/repo",
        "label": "Build",
        "session_id": "s"
    }))
    .unwrap();
    assert_eq!(full.kind, crate::web::pty_manager::PtyKind::Opencode);
    assert_eq!(full.rows, Some(40));
    assert_eq!(full.cols, Some(120));
    assert_eq!(full.project.as_deref(), Some("/repo"));
    assert_eq!(full.label.as_deref(), Some("Build"));
    assert_eq!(full.session_id.as_deref(), Some("s"));

    let minimal: SpawnPtyRequest =
        serde_json::from_value(json!({"kind": "shell", "id": "p2"})).unwrap();
    assert!(minimal.rows.is_none());
    assert!(minimal.cols.is_none());
    assert!(minimal.session_id.is_none());
    assert!(
        minimal.project.is_none() && minimal.label.is_none(),
        "a caller with no pane of its own falls back to the active project"
    );
}

#[test]
fn pty_write_resize_kill_requests() {
    let w: PtyWriteRequest = serde_json::from_value(json!({"id": "p", "data": "YQ=="})).unwrap();
    assert_eq!(w.id, "p");
    assert_eq!(w.data, "YQ==");

    let r: PtyResizeRequest =
        serde_json::from_value(json!({"id": "p", "rows": 10, "cols": 20})).unwrap();
    assert_eq!(r.rows, 10);
    assert_eq!(r.cols, 20);

    let k: PtyKillRequest = serde_json::from_value(json!({"id": "p"})).unwrap();
    assert_eq!(k.id, "p");
}

#[test]
fn sse_token_query_all_optional() {
    let q: SseTokenQuery = serde_json::from_value(json!({"token": "t", "id": "x"})).unwrap();
    assert_eq!(q.token.as_deref(), Some("t"));
    assert_eq!(q.id.as_deref(), Some("x"));
    let empty: SseTokenQuery = serde_json::from_value(json!({})).unwrap();
    assert!(empty.token.is_none());
    assert!(empty.id.is_none());
    // An unasked stream never replays.
    assert_eq!(empty.replay, Replay::No);
}

#[test]
fn sse_token_query_parses_replay_flag() {
    let on: SseTokenQuery = serde_json::from_value(json!({"replay": "1"})).unwrap();
    assert_eq!(on.replay, Replay::Yes);
    let off: SseTokenQuery = serde_json::from_value(json!({"replay": "0"})).unwrap();
    assert_eq!(off.replay, Replay::No);
    let worded: SseTokenQuery = serde_json::from_value(json!({"replay": "true"})).unwrap();
    assert_eq!(worded.replay, Replay::Yes);
}

#[test]
fn model_ref_rename_roundtrip() {
    let m = ModelRef {
        provider_id: "anthropic".into(),
        model_id: "claude".into(),
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["providerID"], "anthropic");
    assert_eq!(v["modelID"], "claude");
    let back: ModelRef = serde_json::from_value(v).unwrap();
    assert_eq!(back.provider_id, "anthropic");
    assert_eq!(back.model_id, "claude");
    assert!(format!("{:?}", back.clone()).contains("ModelRef"));
}

#[test]
fn send_message_request_roundtrip_and_skip() {
    let full = SendMessageRequest {
        parts: vec![json!({"type": "text", "text": "hi"})],
        model: Some(ModelRef {
            provider_id: "p".into(),
            model_id: "m".into(),
        }),
        agent: Some("coder".into()),
        runner: None,
        effort: Some("high".into()),
        permission: Some("on-request".into()),
    };
    let v = serde_json::to_value(&full).unwrap();
    assert_eq!(v["parts"][0]["text"], "hi");
    assert_eq!(v["model"]["providerID"], "p");
    assert_eq!(v["agent"], "coder");
    assert_eq!(v["effort"], "high");
    assert_eq!(v["permission"], "on-request");

    let minimal = SendMessageRequest {
        parts: vec![],
        model: None,
        agent: None,
        runner: None,
        effort: None,
        permission: None,
    };
    let v2 = serde_json::to_value(&minimal).unwrap();
    assert!(v2.get("model").is_none());
    assert!(v2.get("agent").is_none());
    assert!(v2.get("effort").is_none());
    assert!(v2.get("permission").is_none());
    let back: SendMessageRequest = serde_json::from_value(v2).unwrap();
    assert!(back.parts.is_empty());
    assert!(back.model.is_none());
    assert!(back.agent.is_none());
    assert!(back.effort.is_none());
    assert!(back.permission.is_none());
    assert!(back.runner.is_none());
}

#[test]
fn execute_command_request_defaults() {
    let full: ExecuteCommandRequest = serde_json::from_value(json!({
        "command": "/compact",
        "arguments": "arg",
        "model": "opus"
    }))
    .unwrap();
    assert_eq!(full.command, "/compact");
    assert_eq!(full.arguments, "arg");
    assert_eq!(full.model.as_deref(), Some("opus"));

    let minimal: ExecuteCommandRequest =
        serde_json::from_value(json!({"command": "/init", "model": null})).unwrap();
    assert_eq!(minimal.arguments, ""); // serde default
    assert!(minimal.model.is_none());
}

#[test]
fn permission_and_question_reply_requests() {
    let p: PermissionReplyRequest = serde_json::from_value(json!({"reply": "once"})).unwrap();
    assert_eq!(p.reply, "once");
    let q: QuestionReplyRequest =
        serde_json::from_value(json!({"answers": [["a", "b"], ["c"]]})).unwrap();
    assert_eq!(q.answers.len(), 2);
    assert_eq!(q.answers[0], vec!["a", "b"]);
}

#[test]
fn rename_session_request() {
    let r: RenameSessionRequest = serde_json::from_value(json!({"title": "New"})).unwrap();
    assert_eq!(r.title, "New");
}

#[test]
fn a2ui_callback_request_default_payload() {
    let full: A2uiCallbackRequest = serde_json::from_value(json!({
        "callback_id": "cb",
        "payload": {"field": 1}
    }))
    .unwrap();
    assert_eq!(full.callback_id, "cb");
    assert_eq!(full.payload["field"], 1);

    let minimal: A2uiCallbackRequest =
        serde_json::from_value(json!({"callback_id": "cb"})).unwrap();
    assert!(minimal.payload.is_null()); // serde default Value == Null
}

#[test]
fn session_sse_query() {
    let q: SessionSseQuery =
        serde_json::from_value(json!({"token": "t", "project_dir": "/p"})).unwrap();
    assert_eq!(q.token.as_deref(), Some("t"));
    assert_eq!(q.project_dir.as_deref(), Some("/p"));
    let empty: SessionSseQuery = serde_json::from_value(json!({})).unwrap();
    assert!(empty.token.is_none());
    assert!(empty.project_dir.is_none());
}
