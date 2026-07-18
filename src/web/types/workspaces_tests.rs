use super::*;
use serde_json::json;

#[test]
fn workspace_layout_default_and_roundtrip() {
    let d = WorkspaceLayout::default();
    assert_eq!(d.sidebar_width, 0);
    assert_eq!(d.terminal_height, 0);
    assert_eq!(d.side_panel_width, 0);

    let l = WorkspaceLayout {
        sidebar_width: 300,
        terminal_height: 200,
        side_panel_width: 400,
    };
    let v = serde_json::to_value(&l).unwrap();
    assert_eq!(v["sidebar_width"], 300);
    let back: WorkspaceLayout = serde_json::from_value(v).unwrap();
    assert_eq!(back.terminal_height, 200);
    assert_eq!(back.side_panel_width, 400);

    // Defaults fill absent fields.
    let partial: WorkspaceLayout = serde_json::from_value(json!({})).unwrap();
    assert_eq!(partial.sidebar_width, 0);
    assert!(format!("{:?}", back.clone()).contains("WorkspaceLayout"));
}

#[test]
fn workspace_panels_roundtrip() {
    let p = WorkspacePanels {
        sidebar: true,
        terminal: false,
        editor: true,
        git: false,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["sidebar"], true);
    assert_eq!(v["terminal"], false);
    assert_eq!(v["editor"], true);
    assert_eq!(v["git"], false);
    let back: WorkspacePanels = serde_json::from_value(v).unwrap();
    assert!(back.sidebar);
    assert!(format!("{:?}", back.clone()).contains("WorkspacePanels"));
}

#[test]
fn workspace_terminal_tab_default_kind() {
    let t: WorkspaceTerminalTab = serde_json::from_value(json!({"label": "sh"})).unwrap();
    assert_eq!(t.label, "sh");
    assert_eq!(t.kind, ""); // serde default
    let full = WorkspaceTerminalTab {
        label: "cmd".into(),
        kind: "command".into(),
    };
    let v = serde_json::to_value(&full).unwrap();
    assert_eq!(v["kind"], "command");
    assert!(format!("{:?}", full.clone()).contains("WorkspaceTerminalTab"));
}

#[test]
fn workspace_snapshot_full_roundtrip() {
    let snap = WorkspaceSnapshot {
        name: "ws".into(),
        created_at: "2026-01-01".into(),
        panels: WorkspacePanels {
            sidebar: true,
            terminal: true,
            editor: true,
            git: true,
        },
        layout: WorkspaceLayout {
            sidebar_width: 100,
            terminal_height: 50,
            side_panel_width: 75,
        },
        open_files: vec!["a.rs".into(), "b.rs".into()],
        active_file: Some("a.rs".into()),
        terminal_tabs: vec![WorkspaceTerminalTab {
            label: "sh".into(),
            kind: "shell".into(),
        }],
        session_id: Some("s".into()),
        git_branch: Some("main".into()),
        is_template: true,
        recipe_description: Some("desc".into()),
        recipe_next_action: Some("next".into()),
        is_recipe: true,
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["name"], "ws");
    assert_eq!(v["open_files"][1], "b.rs");
    assert_eq!(v["active_file"], "a.rs");
    assert_eq!(v["layout"]["sidebar_width"], 100);
    assert_eq!(v["is_template"], true);
    assert_eq!(v["recipe_description"], "desc");
    assert_eq!(v["is_recipe"], true);
    let back: WorkspaceSnapshot = serde_json::from_value(v).unwrap();
    assert_eq!(back.terminal_tabs.len(), 1);
    assert_eq!(back.session_id.as_deref(), Some("s"));
    assert!(format!("{:?}", back.clone()).contains("WorkspaceSnapshot"));
}

#[test]
fn workspace_snapshot_minimal_defaults() {
    let snap: WorkspaceSnapshot = serde_json::from_value(json!({
        "name": "w",
        "created_at": "t",
        "panels": {"sidebar": false, "terminal": false, "editor": false, "git": false}
    }))
    .unwrap();
    assert_eq!(snap.layout.sidebar_width, 0);
    assert!(snap.open_files.is_empty());
    assert!(snap.active_file.is_none());
    assert!(snap.terminal_tabs.is_empty());
    assert!(snap.session_id.is_none());
    assert!(snap.git_branch.is_none());
    assert!(!snap.is_template);
    assert!(snap.recipe_description.is_none());
    assert!(snap.recipe_next_action.is_none());
    assert!(!snap.is_recipe);
}

#[test]
fn save_workspace_request_deserialize() {
    let req: SaveWorkspaceRequest = serde_json::from_value(json!({
        "snapshot": {
            "name": "w",
            "created_at": "t",
            "panels": {"sidebar": true, "terminal": false, "editor": false, "git": false}
        }
    }))
    .unwrap();
    assert_eq!(req.snapshot.name, "w");
    assert!(format!("{:?}", req.clone()).contains("SaveWorkspaceRequest"));
}

#[test]
fn workspaces_list_response_serialize() {
    let resp = WorkspacesListResponse { workspaces: vec![] };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v["workspaces"].as_array().unwrap().is_empty());
    assert!(format!("{:?}", resp.clone()).contains("WorkspacesListResponse"));
}
