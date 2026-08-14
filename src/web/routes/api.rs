use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;

use super::super::handlers;
use super::super::editor_ws;
use super::super::mcp_ws;
use super::super::sse;

pub(super) fn api_routes() -> Router<super::super::types::ServerState> {
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
        .route("/pty/rename", post(handlers::pty_rename))
        .route("/pty/kill", post(handlers::pty_kill))
        .route("/pty/sessions", get(handlers::pty_sessions))
        .route("/pty/stream", get(sse::terminal_stream))
        // Browser panes: a headless Chromium tab per browser widget. Reads go out as a
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
        .route("/browser/stream", get(super::super::browser_sse::browser_stream))
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
        .route("/editor/events", get(sse::editor_events_stream))
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
        .route("/git/repos", get(handlers::git_repos))
        .route("/git/pull", post(handlers::git_pull))
        .route("/git/stash", post(handlers::git_stash))
        .route("/git/gitignore", post(handlers::git_gitignore))
        // ── File browsing / editing ──────────────────────────────────
        .route("/agents", get(handlers::get_agents))
        .route("/files", get(handlers::browse_files))
        .route("/files/search", get(handlers::search_files))
        .route("/file/read", get(handlers::read_file))
        .route("/file/raw", get(handlers::read_file_raw))
        .route("/file/write", post(handlers::write_file))
        .route("/file/doc-read", get(handlers::doc_read))
        .route("/file/doc-write", post(handlers::doc_write))
        .route("/file/create", post(handlers::create_file))
        .route("/file/delete", post(handlers::delete_file))
        .route("/file/upload", post(handlers::upload_files))
        .route("/file/download", get(handlers::download_file))
        .route("/rename", post(handlers::rename_entry))
        .route("/dir/create", post(handlers::create_dir))
        .route("/dir/delete", post(handlers::delete_dir))
        .route("/dir/download", get(handlers::download_dir))
        // POST rather than GET: these carry the editor's unsaved buffer, which
        // does not fit in a query string.
        .route(
            "/editor/lsp/diagnostics",
            post(handlers::editor_lsp_diagnostics),
        )
        .route("/editor/lsp/hover", post(handlers::editor_lsp_hover))
        .route(
            "/editor/lsp/definition",
            post(handlers::editor_lsp_definition),
        )
        .route(
            "/editor/lsp/completion",
            post(handlers::editor_lsp_completion),
        )
        .route(
            "/editor/lsp/references",
            post(handlers::editor_lsp_references),
        )
        .route("/editor/lsp/rename", post(handlers::editor_lsp_rename))
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
        .route("/memory/active", get(handlers::list_active_memory))
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
        // ── MCP WebSocket (AI agent tool bridge) ─────────────────────
        .route("/mcp/ws", get(mcp_ws::websocket_handler))
        // The editor's binary channel: every LSP query for one pane, multiplexed.
        .route("/editor/ws", get(editor_ws::websocket_handler))
        // ── MCP Skills ───────────────────────────────────────────────
        .route("/mcp/servers", get(handlers::list_servers))
        .route(
            "/mcp/servers/{name}",
            put(handlers::upsert_server).delete(handlers::delete_server),
        )
        .route("/mcp/servers/{name}/enabled", post(handlers::set_enabled))
        .route("/mcp/servers/{name}/tools", get(handlers::list_tools))
        .route("/mcp/servers/{name}/login", post(handlers::start_login))
        .route(
            "/mcp/servers/{name}/login/finish",
            post(handlers::finish_login),
        )
        .route("/mcp/servers/{name}/logout", post(handlers::logout_server))
        // ── ACP agents (the runners themselves) ──────────────────────
        .route(
            "/acp/agents",
            get(handlers::list_agents).delete(handlers::reset_agents),
        )
        .route(
            "/acp/agents/{id}",
            put(handlers::upsert_agent).delete(handlers::delete_agent),
        )
        .route(
            "/acp/agents/{id}/enabled",
            post(handlers::set_agent_enabled),
        )
        .route(
            "/skills",
            get(handlers::list_skills).post(handlers::create_skill),
        )
        .route(
            "/skills/{name}",
            get(handlers::get_skill)
                .put(handlers::update_skill)
                .delete(handlers::delete_skill),
        )
        .route("/skills/upload", post(handlers::upload_skills))
        // ── System Monitor ──────────────────────────────────────────
        .route("/system/stats", get(handlers::get_system_stats))
        .route("/system/stats/stream", get(sse::system_stats_stream))
        // ── Process Health ─────────────────────────────────────────
        .route("/health/status", get(handlers::get_health_status))
        .route("/health/audit", get(handlers::get_health_audit))
        .route("/health/toggle", post(handlers::toggle_health_mitigation))
        .route("/health/config", post(handlers::set_health_config))
        // ── Kanban board ─────────────────────────────────────────────
        .route("/kanban/board", get(handlers::get_board))
        .route(
            "/kanban/board/{board_id}/config",
            axum::routing::put(handlers::update_board_config),
        )
        .route("/kanban/task", post(handlers::create_task))
        .route(
            "/kanban/task/{task_id}",
            get(handlers::get_task)
                .patch(handlers::update_task)
                .delete(handlers::delete_task),
        )
        .route("/kanban/task/{task_id}/launch", post(handlers::launch_task))
        .route("/kanban/task/{task_id}/abort", post(handlers::abort_task))
        .route("/kanban/task/{task_id}/note", post(handlers::add_user_note))
        .route(
            "/kanban/asset/{task_id}/{filename}",
            get(handlers::serve_asset),
        );

    // Attachment upload gets a larger body limit (videos up to ~200 MB).
    let kanban_upload = Router::new()
        .route(
            "/kanban/task/{task_id}/attachment",
            post(handlers::upload_attachment),
        )
        .layer(DefaultBodyLimit::max(220 * 1024 * 1024));

    let api_routes = api_routes.merge(kanban_upload);
    api_routes
}
