use super::*;
use crate::theme::ThemeColors;
use crate::web::types::{
    ActivityEventPayload, PresenceSnapshot, WatcherStatusEvent, WebSessionStats,
};
use ratatui::style::Color;
use serde_json::json;

fn stats() -> WebSessionStats {
    WebSessionStats {
        session_id: "s".into(),
        cost: 1.0,
        input_tokens: 2,
        output_tokens: 3,
        reasoning_tokens: 4,
        cache_read: 5,
        cache_write: 6,
    }
}

#[test]
fn web_event_simple_variants_tagged() {
    assert_eq!(
        serde_json::to_value(WebEvent::StateChanged).unwrap(),
        json!({"type": "StateChanged"})
    );
    assert_eq!(
        serde_json::to_value(WebEvent::RoutineUpdated).unwrap(),
        json!({"type": "RoutineUpdated"})
    );
    assert_eq!(
        serde_json::to_value(WebEvent::Noop).unwrap(),
        json!({"type": "Noop"})
    );
}

#[test]
fn web_event_session_variants() {
    let v = serde_json::to_value(WebEvent::SessionBusy {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v, json!({"type": "SessionBusy", "session_id": "s"}));

    let v = serde_json::to_value(WebEvent::SessionIdle {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "SessionIdle");

    let v = serde_json::to_value(WebEvent::SessionError {
        session_id: "s".into(),
        message: "boom".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "SessionError");
    assert_eq!(v["message"], "boom");

    let v = serde_json::to_value(WebEvent::SessionInputNeeded {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "SessionInputNeeded");

    let v = serde_json::to_value(WebEvent::SessionInputCleared {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "SessionInputCleared");

    let v = serde_json::to_value(WebEvent::SessionUnseen {
        session_id: "s".into(),
        count: 5,
    })
    .unwrap();
    assert_eq!(v["count"], 5);

    let v = serde_json::to_value(WebEvent::SessionSeen {
        session_id: "s".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "SessionSeen");
}

#[test]
fn web_event_stats_and_theme() {
    let v = serde_json::to_value(WebEvent::StatsUpdated(stats())).unwrap();
    assert_eq!(v["type"], "StatsUpdated");
    assert_eq!(v["cost"], 1.0);

    let pair = WebThemePair {
        dark: WebThemeColors::from_theme(&ThemeColors::default()),
        light: WebThemeColors::from_theme(&ThemeColors::default()),
    };
    let v = serde_json::to_value(WebEvent::ThemeChanged(pair)).unwrap();
    assert_eq!(v["type"], "ThemeChanged");
    assert!(v["dark"].is_object());
}

#[test]
fn web_event_watcher_and_mcp_variants() {
    let v = serde_json::to_value(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
        session_id: "s".into(),
        action: "created".into(),
        idle_since_secs: Some(10),
    }))
    .unwrap();
    assert_eq!(v["type"], "WatcherStatusChanged");
    assert_eq!(v["idle_since_secs"], 10);

    // McpEditorOpen with line present and absent (skip_serializing_if).
    let v = serde_json::to_value(WebEvent::McpEditorOpen {
        path: "a.rs".into(),
        line: Some(42),
    })
    .unwrap();
    assert_eq!(v["line"], 42);
    let v = serde_json::to_value(WebEvent::McpEditorOpen {
        path: "a.rs".into(),
        line: None,
    })
    .unwrap();
    assert!(v.get("line").is_none());

    let v = serde_json::to_value(WebEvent::McpEditorNavigate { line: 3 }).unwrap();
    assert_eq!(v["line"], 3);

    let v = serde_json::to_value(WebEvent::McpTerminalFocus { id: "t".into() }).unwrap();
    assert_eq!(v["id"], "t");

    let v = serde_json::to_value(WebEvent::McpAgentActivity {
        tool: "Bash".into(),
        active: true,
    })
    .unwrap();
    assert_eq!(v["tool"], "Bash");
    assert_eq!(v["active"], true);
}

#[test]
fn web_event_activity_presence_mission() {
    let v = serde_json::to_value(WebEvent::ActivityEvent(ActivityEventPayload {
        session_id: "s".into(),
        kind: "k".into(),
        summary: "sum".into(),
        detail: None,
        timestamp: "t".into(),
    }))
    .unwrap();
    assert_eq!(v["type"], "ActivityEvent");

    let v = serde_json::to_value(WebEvent::PresenceChanged(PresenceSnapshot {
        clients: vec![],
    }))
    .unwrap();
    assert_eq!(v["type"], "PresenceChanged");

    let v = serde_json::to_value(WebEvent::MissionUpdated {
        mission: json!({"id": "m"}),
    })
    .unwrap();
    assert_eq!(v["mission"]["id"], "m");
}

#[test]
fn web_event_kanban_and_toast() {
    let v = serde_json::to_value(WebEvent::KanbanTaskUpdated {
        project_path: "/p".into(),
        task_id: "t".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "KanbanTaskUpdated");
    assert_eq!(v["project_path"], "/p");

    let v = serde_json::to_value(WebEvent::KanbanBoardUpdated {
        project_path: "/p".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "KanbanBoardUpdated");

    let v = serde_json::to_value(WebEvent::Toast {
        message: "hi".into(),
        level: "info".into(),
    })
    .unwrap();
    assert_eq!(v["message"], "hi");
    assert_eq!(v["level"], "info");

    // Clone + Debug coverage.
    let e = WebEvent::StateChanged;
    let _ = e.clone();
    let _ = format!("{e:?}");
}

#[test]
fn editor_event_serializes() {
    let v = serde_json::to_value(EditorEvent::FileChanged {
        path: "rel/a.rs".into(),
        source: "web_save".into(),
    })
    .unwrap();
    assert_eq!(
        v,
        json!({"type": "FileChanged", "path": "rel/a.rs", "source": "web_save"})
    );
    let e = EditorEvent::FileChanged {
        path: "p".into(),
        source: "ai_edit".into(),
    };
    let _ = e.clone();
    let _ = format!("{e:?}");
}

#[test]
fn web_theme_colors_from_default_theme_hex() {
    let c = WebThemeColors::from_theme(&ThemeColors::default());
    assert_eq!(c.primary, "#fab283");
    assert_eq!(c.text_muted, "#808080");
    assert_eq!(c.info, "#56b6c2");
    // Roundtrip serialize/deserialize.
    let v = serde_json::to_value(&c).unwrap();
    let back: WebThemeColors = serde_json::from_value(v).unwrap();
    assert_eq!(back.primary, "#fab283");
    let _ = format!("{c:?}");
    let _ = c.clone();
}

#[test]
fn web_theme_colors_non_rgb_falls_back_to_grey() {
    let mut t = ThemeColors::default();
    t.primary = Color::Reset; // non-RGB → fallback branch in color_to_hex
    t.error = Color::White;
    let c = WebThemeColors::from_theme(&t);
    assert_eq!(c.primary, "#808080");
    assert_eq!(c.error, "#808080");
    // Other fields still convert normally.
    assert_eq!(c.secondary, "#5c9cf5");
}

#[test]
fn theme_preview_serializes() {
    let p = ThemePreview {
        name: "opencode".into(),
        dark: WebThemeColors::from_theme(&ThemeColors::default()),
        light: WebThemeColors::from_theme(&ThemeColors::default()),
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["name"], "opencode");
    assert!(v["dark"].is_object());
    assert!(v["light"].is_object());
    let _ = p.clone();
}

#[test]
fn web_theme_pair_roundtrip() {
    let pair = WebThemePair {
        dark: WebThemeColors::from_theme(&ThemeColors::default()),
        light: WebThemeColors::from_theme(&ThemeColors::default()),
    };
    let v = serde_json::to_value(&pair).unwrap();
    let back: WebThemePair = serde_json::from_value(v).unwrap();
    assert_eq!(back.dark.primary, "#fab283");
    let _ = format!("{pair:?}");
    let _ = pair.clone();
}

#[test]
fn web_theme_pair_from_active_theme_structure() {
    // Read-only; returns defaults if no config present. Assert structure, not exact values.
    let pair = WebThemePair::from_active_theme();
    assert!(pair.dark.primary.starts_with('#'));
    assert!(pair.light.primary.starts_with('#'));
    assert_eq!(pair.dark.primary.len(), 7);
    assert_eq!(pair.light.info.len(), 7);
}

#[test]
fn switch_theme_request_deserializes() {
    let r: SwitchThemeRequest = serde_json::from_value(json!({"name": "tokyonight"})).unwrap();
    assert_eq!(r.name, "tokyonight");
}
