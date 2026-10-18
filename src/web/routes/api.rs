use axum::routing::{delete, get, post, put};
use axum::Router;

use super::super::handlers;
use super::super::pty_stream;
use super::super::sse;

#[path = "../handlers/runner_handlers.rs"]
mod runner_handlers;
#[path = "../handlers/runner_limits_handlers.rs"]
mod runner_limits_handlers;
#[path = "../handlers/runner_settings_handlers.rs"]
mod runner_settings_handlers;
#[path = "../handlers/usage_handlers.rs"]
mod usage_handlers;

pub(super) fn api_routes() -> Router<super::super::types::ServerState> {
    let api_routes = Router::new()
        // Auth
        .route("/auth/login", post(handlers::login))
        .route("/auth/verify", get(handlers::verify))
        // State
        .route("/state", get(handlers::get_state))
        .route("/sessions", get(handlers::get_session_page))
        .route(
            "/session/{session_id}/stats",
            get(handlers::get_session_stats),
        )
        .route(
            "/keybindings",
            get(handlers::get_keybindings).put(handlers::put_keybindings),
        )
        .route("/theme", get(handlers::get_theme))
        .route("/themes", get(handlers::list_themes))
        .route("/theme/switch", post(handlers::switch_theme))
        // Actions (independent web state)
        .route("/project/switch", post(handlers::switch_project))
        .route("/project/add", post(handlers::add_project))
        .route("/project/remove", post(handlers::remove_project))
        .route("/runner/select", post(runner_handlers::select_runner))
        .route("/usage", get(usage_handlers::get_usage))
        .route(
            "/runners/settings",
            get(runner_settings_handlers::get_runner_settings),
        )
        .route(
            "/runners/settings/{id}",
            put(runner_settings_handlers::put_runner_enabled),
        )
        .route(
            "/runners/default",
            put(runner_settings_handlers::put_default_runner),
        )
        .route(
            "/runner-limits",
            get(runner_limits_handlers::get_runner_limits)
                .put(runner_limits_handlers::put_runner_limits),
        )
        // Directory browsing (for add-project picker)
        .route("/dirs/home", get(handlers::home_dir))
        .route("/dirs/browse", post(handlers::browse_dirs))
        .route("/dirs/create", post(handlers::create_project_dir))
        .route("/session/select", post(handlers::select_session))
        .route("/session/new", post(handlers::new_session))
        .route("/panel/toggle", post(handlers::toggle_panel))
        .route("/panel/focus", post(handlers::focus_panel))
        // Web PTY management (independent from TUI)
        .route("/pty/spawn", post(handlers::spawn_pty))
        .route("/pty/write", post(handlers::pty_write))
        .route("/pty/resize", post(handlers::pty_resize))
        .route("/pty/rename", post(handlers::pty_rename))
        .route("/pty/kill", post(handlers::pty_kill))
        .route("/pty/sessions", get(handlers::pty_sessions))
        .route("/pty/stream", get(pty_stream::terminal_stream))
        .route("/pty/ws", get(pty_stream::terminal_socket))
        // Browser panes: a native Chrome tab per browser widget. Reads go out as a
        // compact `[ref=eN]` outline rather than HTML — see `crate::browser`.
        .route("/browser/open", post(handlers::browser_open))
        .route("/browser/navigate", post(handlers::browser_navigate))
        .route("/browser/back", post(handlers::browser_back))
        .route("/browser/forward", post(handlers::browser_forward))
        .route("/browser/reload", post(handlers::browser_reload))
        .route("/browser/snapshot", get(handlers::browser_snapshot))
        .route("/browser/text", get(handlers::browser_text))
        .route("/browser/screenshot", get(handlers::browser_screenshot))
        .route("/browser/click", post(handlers::browser_click))
        .route("/browser/type", post(handlers::browser_type))
        .route("/browser/key", post(handlers::browser_key))
        .route("/browser/scroll", post(handlers::browser_scroll))
        .route("/browser/mouse", post(handlers::browser_mouse))
        .route("/browser/text-input", post(handlers::browser_insert_text))
        .route("/browser/mode", post(handlers::browser_mode))
        .route("/browser/resize", post(handlers::browser_resize))
        .route("/browser/close", post(handlers::browser_close))
        .route("/browser/list", get(handlers::browser_list))
        .route("/browser/device", get(handlers::browser_device))
        .route(
            "/browser/stream",
            get(super::super::browser_sse::browser_stream),
        )
        // App events SSE
        .route("/events", get(sse::events_stream))
        // ── Context Window ───────────────────────────────────────────
        .route("/context-window", get(handlers::get_context_window))
        // ── File Edits / Diff Review ─────────────────────────────────
        .route(
            "/session/{session_id}/file-edits",
            get(handlers::get_file_edits),
        )
        // ── Cross-Session Search ─────────────────────────────────────
        .route(
            "/project/{project_idx}/search",
            get(handlers::search_messages),
        )
        // ── Proxy endpoints (opencode server) ────────────────────────
        .route(
            "/session/{session_id}/messages",
            get(handlers::get_session_messages),
        )
        .route(
            "/session/{session_id}/message",
            post(handlers::send_message),
        )
        .route(
            "/session/{session_id}/engine",
            axum::routing::patch(handlers::set_session_engine),
        )
        .route("/session/{session_id}/abort", post(handlers::abort_session))
        .route(
            "/session/{session_id}",
            delete(handlers::delete_session).patch(handlers::rename_session),
        )
        .route(
            "/session/{session_id}/command",
            post(handlers::execute_command),
        )
        .route(
            "/session/{session_id}/todos",
            get(handlers::get_session_todos).put(handlers::update_session_todos),
        )
        .route(
            "/session/{session_id}/queue",
            get(handlers::get_session_queue).delete(handlers::clear_session_queue),
        )
        .route(
            "/session/{session_id}/queue/{index}",
            delete(handlers::remove_session_queue_item),
        )
        .route(
            "/session/{session_id}/mark_seen",
            post(handlers::mark_session_seen),
        )
        .route(
            "/session/{session_id}/a2ui/callback",
            post(handlers::a2ui_callback),
        )
        // ── Multi-session dashboard ──────────────────────────────────
        .route("/providers", get(handlers::get_providers))
        .route("/commands", get(handlers::get_commands))
        .route(
            "/permission/{request_id}/reply",
            post(handlers::reply_permission),
        )
        .route(
            "/question/{request_id}/reply",
            post(handlers::reply_question),
        )
        .route(
            "/question/{request_id}/reject",
            post(handlers::reject_question),
        )
        .route("/pending", get(handlers::get_pending))
        // Session events SSE (proxied from opencode)
        .route("/session/events", get(sse::session_events_stream))
        // Editor events SSE (file change notifications)
        .route("/editor/events", get(sse::editor_events_stream));
    super::api_extra::routes(api_routes)
}
