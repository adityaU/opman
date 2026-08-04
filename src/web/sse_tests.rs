//! Generated tests for SSE event-rendering helpers and pre-stream guards.
//!
//! The infinite stream loops (heartbeat select, drain-at-20fps, blocking
//! system-stats thread) are not driven here — see the module report. We cover
//! the pure `render_web_event` / `render_editor_event` mapping plus the
//! synchronous guard branches (auth, missing id, PTY not found) that run before
//! any stream is constructed, and confirm the authorized entry points return Ok.

use super::*;
use crate::web::test_support::{test_server_state, test_server_state_with_auth};
use axum::extract::{Query, State};
use axum::http::HeaderMap;

fn theme_colors() -> WebThemeColors {
    let c = || "#101010".to_string();
    WebThemeColors {
        primary: c(),
        secondary: c(),
        accent: c(),
        background: c(),
        background_panel: c(),
        background_element: c(),
        text: c(),
        text_muted: c(),
        border: c(),
        border_active: c(),
        border_subtle: c(),
        error: c(),
        warning: c(),
        success: c(),
        info: c(),
    }
}

fn q(token: Option<&str>, id: Option<&str>) -> Query<SseTokenQuery> {
    Query(SseTokenQuery {
        token: token.map(|s| s.to_string()),
        id: id.map(|s| s.to_string()),
    })
}

// ── render_web_event ────────────────────────────────────────────────

#[test]
fn render_noop_is_none() {
    assert!(render_web_event(&WebEvent::Noop).is_none());
}

#[test]
fn render_simple_string_events_are_some() {
    let cases = [
        WebEvent::StateChanged,
        WebEvent::RoutineUpdated,
        WebEvent::SessionBusy {
            session_id: "s".into(),
        },
        WebEvent::SessionIdle {
            session_id: "s".into(),
        },
        WebEvent::McpTerminalFocus { id: "t".into() },
        WebEvent::SessionSeen {
            session_id: "s".into(),
        },
        WebEvent::SessionInputNeeded {
            session_id: "s".into(),
        },
        WebEvent::SessionInputCleared {
            session_id: "s".into(),
        },
    ];
    for ev in cases {
        assert!(render_web_event(&ev).is_some(), "expected Some for {ev:?}");
    }
}

#[test]
fn render_payload_events_are_some() {
    let cases = [
        WebEvent::McpEditorOpen {
            path: "/a".into(),
            line: Some(3),
        },
        WebEvent::McpEditorOpen {
            path: "/a".into(),
            line: None,
        },
        WebEvent::McpEditorNavigate { line: 5 },
        WebEvent::McpAgentActivity {
            tool: "read".into(),
            active: true,
        },
        WebEvent::MissionUpdated {
            mission: serde_json::json!({"id": "m"}),
        },
        WebEvent::KanbanTaskUpdated {
            project_path: "/p".into(),
            task_id: "t".into(),
        },
        WebEvent::KanbanBoardUpdated {
            project_path: "/p".into(),
        },
        WebEvent::Toast {
            message: "hi".into(),
            level: "info".into(),
        },
        WebEvent::SessionError {
            session_id: "s".into(),
            message: "boom".into(),
        },
        WebEvent::SessionUnseen {
            session_id: "s".into(),
            count: 2,
        },
    ];
    for ev in cases {
        assert!(render_web_event(&ev).is_some(), "expected Some for {ev:?}");
    }
}

#[test]
fn render_serialized_events_are_some() {
    assert!(render_web_event(&WebEvent::StatsUpdated(WebSessionStats::default())).is_some());
    assert!(render_web_event(&WebEvent::ThemeChanged(WebThemePair {
        dark: theme_colors(),
        light: theme_colors(),
    }))
    .is_some());
    assert!(
        render_web_event(&WebEvent::WatcherStatusChanged(WatcherStatusEvent {
            session_id: "s".into(),
            action: "created".into(),
            idle_since_secs: Some(4),
        }))
        .is_some()
    );
    assert!(
        render_web_event(&WebEvent::ActivityEvent(ActivityEventPayload {
            session_id: "s".into(),
            kind: "status".into(),
            summary: "done".into(),
            detail: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
        }))
        .is_some()
    );
    assert!(
        render_web_event(&WebEvent::PresenceChanged(PresenceSnapshot {
            clients: vec![]
        }))
        .is_some()
    );
}

#[test]
fn render_editor_event_is_some() {
    let ev = EditorEvent::FileChanged {
        path: "a.rs".into(),
        source: "web_save".into(),
    };
    assert!(render_editor_event(&ev).is_some());
}

// ── terminal_stream guards ──────────────────────────────────────────

#[tokio::test]
async fn terminal_stream_rejects_bad_auth() {
    let state = test_server_state_with_auth("u", "p");
    let res = terminal_stream(State(state), HeaderMap::new(), q(None, Some("x"))).await;
    assert!(matches!(res.err(), Some(WebError::Unauthorized)));
}

#[tokio::test]
async fn terminal_stream_missing_id_is_bad_request() {
    let state = test_server_state(); // empty creds -> auth passes
    let res = terminal_stream(State(state), HeaderMap::new(), q(None, None)).await;
    assert!(matches!(res.err(), Some(WebError::BadRequest(_))));
}

#[tokio::test]
async fn terminal_stream_unknown_pty_is_not_found() {
    let state = test_server_state();
    // no-op pty handle -> get_output returns None.
    let res = terminal_stream(State(state), HeaderMap::new(), q(None, Some("missing"))).await;
    assert!(matches!(res.err(), Some(WebError::NotFound(_))));
}

// ── other streams: auth guard + authorized entry returns Ok ─────────

#[tokio::test]
async fn events_stream_auth_and_ok() {
    let denied = events_stream(
        State(test_server_state_with_auth("u", "p")),
        HeaderMap::new(),
        q(None, None),
    )
    .await;
    assert!(denied.is_err());

    // Empty creds -> passes auth, subscribes, returns Ok (stream is lazy).
    let ok = events_stream(State(test_server_state()), HeaderMap::new(), q(None, None)).await;
    assert!(ok.is_ok());
}

#[tokio::test]
async fn session_events_stream_auth_and_ok() {
    let denied = session_events_stream(
        State(test_server_state_with_auth("u", "p")),
        HeaderMap::new(),
        Query(SessionSseQuery {
            token: None,
            project_dir: None,
        }),
    )
    .await;
    assert!(denied.is_err());

    let ok = session_events_stream(
        State(test_server_state()),
        HeaderMap::new(),
        Query(SessionSseQuery {
            token: None,
            project_dir: Some("/p".into()),
        }),
    )
    .await;
    assert!(ok.is_ok());
}

#[tokio::test]
async fn editor_events_stream_auth_and_ok() {
    let denied = editor_events_stream(
        State(test_server_state_with_auth("u", "p")),
        HeaderMap::new(),
        q(None, None),
    )
    .await;
    assert!(denied.is_err());

    let ok =
        editor_events_stream(State(test_server_state()), HeaderMap::new(), q(None, None)).await;
    assert!(ok.is_ok());
}

#[tokio::test]
async fn system_stats_stream_rejects_bad_auth() {
    // Only the auth-reject path is exercised; the authorized path spawns a
    // persistent blocking sampling thread, which we avoid in tests.
    let denied = system_stats_stream(
        State(test_server_state_with_auth("u", "p")),
        HeaderMap::new(),
        q(None, None),
    )
    .await;
    assert!(matches!(denied.err(), Some(WebError::Unauthorized)));
}
