use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use axum::Router;

use super::handlers;
use super::mcp_ws;
use super::sse;
use super::static_files;
use super::types::ServerState;

pub(super) fn build_router(state: ServerState) -> Router {
    let api_routes = Router::new()
        // Auth
        .route("/auth/login", post(handlers::login))
        .route("/auth/verify", get(handlers::verify))
        // State
        .route("/state", get(handlers::get_state))
        .route(
            "/session/{session_id}/stats",
            get(handlers::get_session_stats),
        )
        .route("/theme", get(handlers::get_theme))
        .route("/themes", get(handlers::list_themes))
        .route("/theme/switch", post(handlers::switch_theme))
        // Actions (independent web state)
        .route("/project/switch", post(handlers::switch_project))
        .route("/project/add", post(handlers::add_project))
        .route("/project/remove", post(handlers::remove_project))
        // Directory browsing (for add-project picker)
        .route("/dirs/home", get(handlers::home_dir))
        .route("/dirs/browse", post(handlers::browse_dirs))
        .route("/session/select", post(handlers::select_session))
        .route("/session/new", post(handlers::new_session))
        .route("/panel/toggle", post(handlers::toggle_panel))
        .route("/panel/focus", post(handlers::focus_panel))
        // Web PTY management (independent from TUI)
        .route("/pty/spawn", post(handlers::spawn_pty))
        .route("/pty/write", post(handlers::pty_write))
        .route("/pty/resize", post(handlers::pty_resize))
        .route("/pty/kill", post(handlers::pty_kill))
        .route("/pty/list", get(handlers::pty_list))
        .route("/pty/stream", get(sse::terminal_stream))
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
            get(handlers::get_session_todos),
        )
        // ── Multi-session dashboard ──────────────────────────────────
        .route("/sessions/overview", get(handlers::sessions_overview))
        .route("/sessions/tree", get(handlers::sessions_tree))
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
        // Session events SSE (proxied from opencode)
        .route("/session/events", get(sse::session_events_stream))
        // ── Git API (shell out to git CLI) ───────────────────────────
        .route("/git/status", get(handlers::git_status))
        .route("/git/diff", get(handlers::git_diff))
        .route("/git/log", get(handlers::git_log))
        .route("/git/stage", post(handlers::git_stage))
        .route("/git/unstage", post(handlers::git_unstage))
        .route("/git/commit", post(handlers::git_commit))
        .route("/git/discard", post(handlers::git_discard))
        .route("/git/show", get(handlers::git_show))
        .route("/git/branches", get(handlers::git_branches))
        .route("/git/checkout", post(handlers::git_checkout))
        .route("/git/range-diff", get(handlers::git_range_diff))
        .route("/git/context-summary", get(handlers::git_context_summary))
        // ── File browsing / editing ──────────────────────────────────
        .route("/agents", get(handlers::get_agents))
        .route("/files", get(handlers::browse_files))
        .route("/file/read", get(handlers::read_file))
        .route("/file/raw", get(handlers::read_file_raw))
        .route("/file/write", post(handlers::write_file))
        .route(
            "/editor/lsp/diagnostics",
            get(handlers::editor_lsp_diagnostics),
        )
        .route("/editor/lsp/hover", get(handlers::editor_lsp_hover))
        .route(
            "/editor/lsp/definition",
            get(handlers::editor_lsp_definition),
        )
        .route("/editor/lsp/format", post(handlers::editor_lsp_format))
        // ── Session Watcher ──────────────────────────────────────────
        .route("/watchers", get(handlers::list_watchers))
        .route("/watcher", post(handlers::create_watcher))
        .route("/watcher/sessions", get(handlers::get_watcher_sessions))
        .route(
            "/watcher/{session_id}",
            get(handlers::get_watcher).delete(handlers::delete_watcher),
        )
        .route(
            "/watcher/{session_id}/messages",
            get(handlers::get_watcher_messages),
        )
        // ── Session Continuity: Presence + Activity ──────────────────
        .route(
            "/presence",
            get(handlers::get_presence)
                .post(handlers::register_presence)
                .delete(handlers::deregister_presence),
        )
        .route("/activity", get(handlers::get_activity_feed))
        // ── Missions ───────────────────────────────────────────────
        .route(
            "/missions",
            get(handlers::list_missions).post(handlers::create_mission),
        )
        .route(
            "/missions/{mission_id}",
            axum::routing::patch(handlers::update_mission).delete(handlers::delete_mission),
        )
        // ── Personal Memory ─────────────────────────────────────
        .route(
            "/memory",
            get(handlers::list_personal_memory).post(handlers::create_personal_memory),
        )
        .route(
            "/memory/{memory_id}",
            axum::routing::patch(handlers::update_personal_memory)
                .delete(handlers::delete_personal_memory),
        )
        // ── Autonomy Controls ──────────────────────────────────
        .route(
            "/autonomy",
            get(handlers::get_autonomy_settings).post(handlers::update_autonomy_settings),
        )
        // ── Routines ───────────────────────────────────────────
        .route(
            "/routines",
            get(handlers::list_routines).post(handlers::create_routine),
        )
        .route(
            "/routines/{routine_id}",
            axum::routing::patch(handlers::update_routine).delete(handlers::delete_routine),
        )
        .route("/routines/{routine_id}/run", post(handlers::run_routine))
        // ── Delegation Board ─────────────────────────────────
        .route(
            "/delegation",
            get(handlers::list_delegated_work).post(handlers::create_delegated_work),
        )
        .route(
            "/delegation/{item_id}",
            axum::routing::patch(handlers::update_delegated_work)
                .delete(handlers::delete_delegated_work),
        )
        // ── Workspace Snapshots ─────────────────────────────────────
        .route(
            "/workspaces",
            get(handlers::list_workspaces)
                .post(handlers::save_workspace)
                .delete(handlers::delete_workspace),
        )
        // ── MCP WebSocket (AI agent tool bridge) ─────────────────────
        .route("/mcp/ws", get(mcp_ws::websocket_handler))
        // ── Computed Intelligence (backend-driven) ───────────────────
        .route("/inbox", post(handlers::compute_inbox))
        .route("/recommendations", post(handlers::compute_recommendations))
        .route("/handoff/mission", post(handlers::compute_mission_handoff))
        .route("/handoff/session", post(handlers::compute_session_handoff))
        .route("/resume-briefing", post(handlers::compute_resume_briefing))
        .route("/daily-summary", post(handlers::compute_daily_summary))
        .route(
            "/signals",
            get(handlers::list_signals).post(handlers::add_signal),
        )
        .route(
            "/assistant-center/stats",
            post(handlers::compute_assistant_stats),
        )
        .route(
            "/workspace-templates",
            get(handlers::list_workspace_templates),
        )
        .route("/memory/active", get(handlers::list_active_memory));

    Router::new()
        .route("/health", get(handlers::health))
        .nest("/api", api_routes)
        .fallback(static_files::serve)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB global body limit
        .with_state(state)
}
