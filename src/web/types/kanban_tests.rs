use super::*;
use serde_json::json;

#[test]
fn lane_defaults_apply() {
    let l: Lane = serde_json::from_value(json!({
        "id": "l1", "name": "Todo", "color": "#fff"
    }))
    .unwrap();
    assert!(l.wip.is_none());
    assert!(!l.terminal);
    assert!(l.agent.is_none());
    assert!(l.model.is_none());
    assert!(l.prompt.is_none());
    let _ = format!("{l:?}");
    let _ = l.clone();
}

#[test]
fn lane_full_roundtrip() {
    let l = Lane {
        id: "l".into(),
        name: "n".into(),
        color: "#000".into(),
        wip: Some(3),
        terminal: true,
        agent: Some("build".into()),
        model: Some("claude".into()),
        prompt: Some("do it".into()),
    };
    let v = serde_json::to_value(&l).unwrap();
    assert_eq!(v["wip"], 3);
    assert_eq!(v["terminal"], true);
    let back: Lane = serde_json::from_value(v).unwrap();
    assert_eq!(back.agent, Some("build".into()));
}

fn sample_board() -> Board {
    default_board("b1".into(), "/proj".into())
}

#[test]
fn board_default_shape() {
    let b = sample_board();
    assert_eq!(b.id, "b1");
    assert_eq!(b.project_path, "/proj");
    assert_eq!(b.lanes.len(), 7);
    // first lane has no backward edge, only forward.
    assert_eq!(b.transitions["lane_todo"], vec!["lane_planning".to_string()]);
    // middle lane has forward + backward.
    assert_eq!(
        b.transitions["lane_planning"],
        vec!["lane_implementing".to_string(), "lane_todo".to_string()]
    );
    // last lane has only backward edge.
    assert_eq!(b.transitions["lane_done"], vec!["lane_inreview".to_string()]);
    let _ = format!("{b:?}");
    let _ = b.clone();
}

#[test]
fn board_transition_allowed() {
    let b = sample_board();
    // same lane always allowed.
    assert!(b.transition_allowed("lane_todo", "lane_todo"));
    // forward edge allowed.
    assert!(b.transition_allowed("lane_todo", "lane_planning"));
    // backward edge allowed.
    assert!(b.transition_allowed("lane_planning", "lane_todo"));
    // non-adjacent not allowed.
    assert!(!b.transition_allowed("lane_todo", "lane_done"));
    // unknown source lane → false.
    assert!(!b.transition_allowed("nope", "lane_todo"));
}

#[test]
fn board_terminal_lane_id() {
    let b = sample_board();
    assert_eq!(b.terminal_lane_id(), Some("lane_inreview"));

    // Board with no terminal lane.
    let mut b2 = sample_board();
    for l in &mut b2.lanes {
        l.terminal = false;
    }
    assert_eq!(b2.terminal_lane_id(), None);
}

#[test]
fn board_lane_lookup() {
    let b = sample_board();
    assert_eq!(b.lane("lane_done").unwrap().name, "Done");
    assert!(b.lane("missing").is_none());
}

#[test]
fn board_roundtrip_with_default_transitions() {
    // transitions omitted → default empty map.
    let b: Board = serde_json::from_value(json!({
        "id": "b", "name": "B", "project_path": "/p",
        "lanes": []
    }))
    .unwrap();
    assert!(b.transitions.is_empty());
    assert!(b.lanes.is_empty());
}

#[test]
fn task_defaults_apply() {
    let t: Task = serde_json::from_value(json!({
        "id": "t1", "board_id": "b", "lane_id": "l", "title": "T",
        "order_index": 1.5, "created_at": "c", "updated_at": "u"
    }))
    .unwrap();
    assert_eq!(t.description, "");
    assert!(t.tags.is_empty());
    assert_eq!(t.priority, "normal");
    assert!(t.session_id.is_none());
    assert!(t.launch_model.is_none());
    assert!(t.launch_agent.is_none());
    assert_eq!(t.run_state, "idle");
    assert!(!t.archived);
    let _ = format!("{t:?}");
    let _ = t.clone();
}

#[test]
fn task_full_roundtrip() {
    let t = Task {
        id: "t".into(),
        board_id: "b".into(),
        lane_id: "l".into(),
        title: "Title".into(),
        description: "desc".into(),
        tags: vec!["x".into()],
        priority: "high".into(),
        order_index: 2.0,
        session_id: Some("s".into()),
        launch_model: Some("m".into()),
        launch_agent: Some("a".into()),
        run_state: "running".into(),
        archived: true,
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["priority"], "high");
    assert_eq!(v["archived"], true);
    assert_eq!(v["run_state"], "running");
    let back: Task = serde_json::from_value(v).unwrap();
    assert_eq!(back.tags, vec!["x".to_string()]);
}

#[test]
fn default_priority_and_run_state_helpers() {
    assert_eq!(default_priority(), "normal");
    assert_eq!(default_run_state(), "idle");
}

#[test]
fn attachment_skips_task_id_and_defaults_url() {
    let a: Attachment = serde_json::from_value(json!({
        "id": "a1",
        "filename": "img.png",
        "mime": "image/png",
        "kind": "image",
        "size_bytes": 100,
        "created_at": "c"
    }))
    .unwrap();
    assert_eq!(a.task_id, "", "task_id is #[serde(skip)] → default empty");
    assert_eq!(a.url, "", "url defaults to empty");
    let full = Attachment {
        id: "a".into(),
        task_id: "t".into(),
        filename: "f".into(),
        mime: "image/png".into(),
        kind: "image".into(),
        size_bytes: 5,
        created_at: "c".into(),
        url: "http://x/a".into(),
    };
    let v = serde_json::to_value(&full).unwrap();
    assert!(v.get("task_id").is_none(), "task_id skipped on serialize");
    assert_eq!(v["url"], "http://x/a");
    let _ = format!("{full:?}");
    let _ = full.clone();
}

#[test]
fn kanban_note_defaults_and_roundtrip() {
    let n: KanbanNote = serde_json::from_value(json!({
        "id": "n1", "author": "agent", "body": "note", "created_at": "c"
    }))
    .unwrap();
    assert!(n.lane_from.is_none());
    assert!(n.lane_to.is_none());
    let full = KanbanNote {
        id: "n".into(),
        author: "user".into(),
        body: "b".into(),
        lane_from: Some("l1".into()),
        lane_to: Some("l2".into()),
        created_at: "c".into(),
    };
    let v = serde_json::to_value(&full).unwrap();
    assert_eq!(v["lane_from"], "l1");
    assert_eq!(v["lane_to"], "l2");
    let _ = format!("{n:?}");
    let _ = n.clone();
}

#[test]
fn board_response_default_pipelines() {
    let resp = BoardResponse {
        board: sample_board(),
        tasks: vec![],
        pipelines: vec![],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["pipelines"], json!([]));
    assert_eq!(v["tasks"], json!([]));
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}

#[test]
fn task_detail_flattens_task() {
    let detail = TaskDetail {
        task: Task {
            id: "t".into(),
            board_id: "b".into(),
            lane_id: "l".into(),
            title: "Title".into(),
            description: String::new(),
            tags: vec![],
            priority: "normal".into(),
            order_index: 0.0,
            session_id: None,
            launch_model: None,
            launch_agent: None,
            run_state: "idle".into(),
            archived: false,
            created_at: "c".into(),
            updated_at: "u".into(),
        },
        notes: vec![],
        attachments: vec![],
    };
    let v = serde_json::to_value(&detail).unwrap();
    // flattened task fields appear at top level.
    assert_eq!(v["id"], "t");
    assert_eq!(v["title"], "Title");
    assert_eq!(v["notes"], json!([]));
    assert_eq!(v["attachments"], json!([]));
    let _ = format!("{detail:?}");
    let _ = detail.clone();
}

#[test]
fn create_task_request_defaults() {
    let r: CreateTaskRequest = serde_json::from_value(json!({
        "board_id": "b", "lane_id": "l", "title": "T"
    }))
    .unwrap();
    assert_eq!(r.description, "");
    assert!(r.tags.is_empty());
    assert_eq!(r.priority, "normal");
    let _ = format!("{r:?}");
}

#[test]
fn update_task_request_all_optional() {
    let empty: UpdateTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert!(empty.title.is_none());
    assert!(empty.description.is_none());
    assert!(empty.tags.is_none());
    assert!(empty.priority.is_none());
    assert!(empty.lane_id.is_none());
    assert!(empty.order_index.is_none());
    assert!(empty.archived.is_none());

    let full: UpdateTaskRequest = serde_json::from_value(json!({
        "title": "T", "description": "d", "tags": ["a"], "priority": "high",
        "lane_id": "l", "order_index": 3.0, "archived": true
    }))
    .unwrap();
    assert_eq!(full.title, Some("T".into()));
    assert_eq!(full.tags, Some(vec!["a".into()]));
    assert_eq!(full.order_index, Some(3.0));
    assert_eq!(full.archived, Some(true));
    let _ = format!("{empty:?}");
}

#[test]
fn board_config_request_defaults() {
    let r: BoardConfigRequest = serde_json::from_value(json!({"lanes": []})).unwrap();
    assert!(r.lanes.is_empty());
    assert!(r.transitions.is_empty());
    let _ = format!("{r:?}");
}

#[test]
fn launch_task_request_defaults() {
    let r: LaunchTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert!(r.model.is_none());
    assert!(r.agent.is_none());
    assert!(r.mode.is_none());
    let r2: LaunchTaskRequest = serde_json::from_value(json!({
        "model": "m", "agent": "a", "mode": "pipeline"
    }))
    .unwrap();
    assert_eq!(r2.mode, Some("pipeline".into()));
    let _ = format!("{r:?}");
}

#[test]
fn user_note_request() {
    let r: UserNoteRequest = serde_json::from_value(json!({"body": "hi"})).unwrap();
    assert_eq!(r.body, "hi");
    let _ = format!("{r:?}");
}

#[test]
fn internal_status_request_defaults() {
    let r: InternalStatusRequest = serde_json::from_value(json!({"lane": "l"})).unwrap();
    assert_eq!(r.lane, "l");
    assert!(r.run_state.is_none());
    let r2: InternalStatusRequest =
        serde_json::from_value(json!({"lane": "l", "run_state": "running"})).unwrap();
    assert_eq!(r2.run_state, Some("running".into()));
    let _ = format!("{r:?}");
}

#[test]
fn internal_note_request_defaults() {
    let r: InternalNoteRequest = serde_json::from_value(json!({"body": "b"})).unwrap();
    assert!(r.lane_from.is_none());
    assert!(r.lane_to.is_none());
    let r2: InternalNoteRequest = serde_json::from_value(json!({
        "body": "b", "lane_from": "l1", "lane_to": "l2"
    }))
    .unwrap();
    assert_eq!(r2.lane_from, Some("l1".into()));
    let _ = format!("{r:?}");
}

#[test]
fn kind_from_mime_classification() {
    assert_eq!(kind_from_mime("image/png"), "image");
    assert_eq!(kind_from_mime("image/jpeg"), "image");
    assert_eq!(kind_from_mime("video/mp4"), "video");
    assert_eq!(kind_from_mime("application/pdf"), "file");
    assert_eq!(kind_from_mime("text/plain"), "file");
    assert_eq!(kind_from_mime(""), "file");
}
