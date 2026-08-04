use super::*;

#[test]
fn web_session_time_serialize() {
    let t = WebSessionTime {
        created: 111,
        updated: 222,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["created"], 111);
    assert_eq!(v["updated"], 222);
    let c = t.clone();
    assert_eq!(c.created, 111);
}

#[test]
fn web_session_info_serialize_with_rename() {
    let info = WebSessionInfo {
        id: "s".into(),
        title: "T".into(),
        parent_id: "par".into(),
        directory: "/d".into(),
        time: WebSessionTime {
            created: 1,
            updated: 2,
        },
        runner: "opencode".into(),
    };
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["id"], "s");
    assert_eq!(v["parentID"], "par"); // renamed field
    assert_eq!(v["directory"], "/d");
    assert_eq!(v["time"]["created"], 1);
    let _ = info.clone();
}

#[test]
fn web_panel_visibility_serialize() {
    let p = WebPanelVisibility {
        sidebar: true,
        terminal_pane: false,
        neovim_pane: true,
        integrated_terminal: false,
        git_panel: true,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["sidebar"], true);
    assert_eq!(v["terminal_pane"], false);
    assert_eq!(v["neovim_pane"], true);
    assert_eq!(v["integrated_terminal"], false);
    assert_eq!(v["git_panel"], true);
    let _ = p.clone();
}

#[test]
fn web_project_info_serialize() {
    let proj = WebProjectInfo {
        name: "proj".into(),
        path: "/p".into(),
        index: 2,
        active_session: Some("s".into()),
        sessions: vec![WebSessionInfo {
            id: "s".into(),
            title: "T".into(),
            parent_id: "".into(),
            directory: "/d".into(),
            time: WebSessionTime {
                created: 0,
                updated: 0,
            },
            runner: "claude".into(),
        }],
        git_branch: "main".into(),
        busy_sessions: vec!["s".into()],
        error_sessions: vec![],
        input_sessions: vec![],
        unseen_sessions: vec!["s".into()],
    };
    let v = serde_json::to_value(&proj).unwrap();
    assert_eq!(v["name"], "proj");
    assert_eq!(v["index"], 2);
    assert_eq!(v["active_session"], "s");
    assert_eq!(v["sessions"][0]["id"], "s");
    assert_eq!(v["git_branch"], "main");
    assert_eq!(v["busy_sessions"][0], "s");
    assert_eq!(v["unseen_sessions"][0], "s");
    let _ = proj.clone();
}

#[test]
fn web_project_info_none_active_session() {
    let proj = WebProjectInfo {
        name: "p".into(),
        path: "/p".into(),
        index: 0,
        active_session: None,
        sessions: vec![],
        git_branch: "".into(),
        busy_sessions: vec![],
        error_sessions: vec![],
        input_sessions: vec![],
        unseen_sessions: vec![],
    };
    let v = serde_json::to_value(&proj).unwrap();
    assert!(v["active_session"].is_null());
}

#[test]
fn web_app_state_serialize_with_and_without_instance_name() {
    let panels = WebPanelVisibility {
        sidebar: true,
        terminal_pane: true,
        neovim_pane: false,
        integrated_terminal: false,
        git_panel: false,
    };
    let with = WebAppState {
        projects: vec![],
        active_project: 1,
        panels: panels.clone(),
        focused: "editor".into(),
        instance_name: Some("box1".into()),
        backend: "claude-code".into(),
        runners: vec!["claude".into(), "codex".into()],
    };
    let v = serde_json::to_value(&with).unwrap();
    assert_eq!(v["active_project"], 1);
    assert_eq!(v["focused"], "editor");
    assert_eq!(v["instance_name"], "box1");
    assert_eq!(v["backend"], "claude-code");
    let _ = with.clone();

    let without = WebAppState {
        projects: vec![],
        active_project: 0,
        panels,
        focused: "".into(),
        instance_name: None,
        backend: "opencode".into(),
        runners: vec!["opencode".into()],
    };
    let v2 = serde_json::to_value(&without).unwrap();
    assert!(v2.get("instance_name").is_none()); // skip_serializing_if None
}
