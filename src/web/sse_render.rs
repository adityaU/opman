use super::super::types::{EditorEvent, WebEvent};
use axum::response::sse::Event as SseEvent;

/// Map a broadcast [`WebEvent`] to the SSE event that should be sent to the
/// client, or `None` when the event produces no output (`Noop`, or a payload
/// that fails to serialize). Pure helper extracted from `events_stream` so the
/// per-event mapping is unit-testable.
pub(super) fn render_web_event(event: &WebEvent) -> Option<SseEvent> {
    match event {
        WebEvent::Noop => None,
        WebEvent::SettingsChanged { section } => Some(
            SseEvent::default()
                .event("settings_changed")
                .data(serde_json::json!({"section":section}).to_string()),
        ),
        WebEvent::PreferencesChanged(value) => Some(
            SseEvent::default()
                .event("preferences_changed")
                .data(value.to_string()),
        ),
        WebEvent::StateChanged => Some(SseEvent::default().event("state_changed").data("")),
        WebEvent::McpServersChanged => {
            Some(SseEvent::default().event("mcp_servers_changed").data(""))
        }
        WebEvent::AcpAgentsChanged => {
            Some(SseEvent::default().event("acp_agents_changed").data(""))
        }
        WebEvent::SessionBusy { session_id } => Some(
            SseEvent::default()
                .event("session_busy")
                .data(session_id.clone()),
        ),
        WebEvent::SessionIdle { session_id } => Some(
            SseEvent::default()
                .event("session_idle")
                .data(session_id.clone()),
        ),
        WebEvent::StatsUpdated(stats) => serde_json::to_string(stats)
            .ok()
            .map(|json| SseEvent::default().event("stats_updated").data(json)),
        WebEvent::ThemeChanged(colors) => serde_json::to_string(colors)
            .ok()
            .map(|json| SseEvent::default().event("theme_changed").data(json)),
        WebEvent::WatcherStatusChanged(watcher_event) => serde_json::to_string(watcher_event)
            .ok()
            .map(|json| SseEvent::default().event("watcher_status").data(json)),
        WebEvent::McpEditorOpen { path, line } => {
            let payload = serde_json::json!({ "path": path, "line": line });
            Some(
                SseEvent::default()
                    .event("mcp_editor_open")
                    .data(payload.to_string()),
            )
        }
        WebEvent::McpBrowserOpen {
            project_path,
            browser_id,
            url,
        } => {
            let payload = serde_json::json!({
                "projectPath": project_path,
                "browserId": browser_id,
                "url": url,
            });
            Some(
                SseEvent::default()
                    .event("mcp_browser_open")
                    .data(payload.to_string()),
            )
        }
        WebEvent::McpEditorNavigate { line } => {
            let payload = serde_json::json!({ "line": line });
            Some(
                SseEvent::default()
                    .event("mcp_editor_navigate")
                    .data(payload.to_string()),
            )
        }
        WebEvent::McpTerminalFocus { id } => Some(
            SseEvent::default()
                .event("mcp_terminal_focus")
                .data(id.clone()),
        ),
        WebEvent::McpAgentActivity { tool, active } => {
            let payload = serde_json::json!({ "tool": tool, "active": active });
            Some(
                SseEvent::default()
                    .event("mcp_agent_activity")
                    .data(payload.to_string()),
            )
        }
        WebEvent::PresenceChanged(snapshot) => serde_json::to_string(snapshot)
            .ok()
            .map(|json| SseEvent::default().event("presence_changed").data(json)),
        WebEvent::RoutineUpdated => Some(SseEvent::default().event("routine_updated").data("")),
        WebEvent::KanbanTaskUpdated {
            project_path,
            task_id,
        } => {
            let payload = serde_json::json!({ "project_path": project_path, "task_id": task_id });
            Some(
                SseEvent::default()
                    .event("kanban_task")
                    .data(payload.to_string()),
            )
        }
        WebEvent::KanbanBoardUpdated { project_path } => {
            let payload = serde_json::json!({ "project_path": project_path });
            Some(
                SseEvent::default()
                    .event("kanban_board")
                    .data(payload.to_string()),
            )
        }
        WebEvent::RunnerLimitsChanged => {
            Some(SseEvent::default().event("runner_limits_changed").data(""))
        }
        WebEvent::SessionHandoff { from, to, runner } => {
            let payload = serde_json::json!({ "from": from, "to": to, "runner": runner });
            Some(
                SseEvent::default()
                    .event("session_handoff")
                    .data(payload.to_string()),
            )
        }
        WebEvent::Toast { message, level } => {
            let payload = serde_json::json!({ "message": message, "level": level });
            Some(SseEvent::default().event("toast").data(payload.to_string()))
        }
        WebEvent::SessionError { session_id, .. } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_error")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionInputNeeded { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_input_needed")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionInputCleared { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_input_cleared")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionUnseen { session_id, count } => {
            let payload = serde_json::json!({ "session_id": session_id, "count": count });
            Some(
                SseEvent::default()
                    .event("session_unseen")
                    .data(payload.to_string()),
            )
        }
        WebEvent::SessionSeen { session_id } => {
            let payload = serde_json::json!({ "session_id": session_id });
            Some(
                SseEvent::default()
                    .event("session_seen")
                    .data(payload.to_string()),
            )
        }
    }
}

/// Map an [`EditorEvent`] to the `file_changed` SSE event, or `None` if it
/// fails to serialize. Pure helper extracted from `editor_events_stream`.
pub(super) fn render_editor_event(event: &EditorEvent) -> Option<SseEvent> {
    serde_json::to_string(event)
        .ok()
        .map(|json| SseEvent::default().event("file_changed").data(json))
}
