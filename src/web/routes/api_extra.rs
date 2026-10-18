use super::super::{editor_ws, handlers, mcp_ws, sse};
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use axum::Router;

pub(super) fn routes(
    router: Router<super::super::types::ServerState>,
) -> Router<super::super::types::ServerState> {
    let api_routes = router
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
        // Branch lifecycle
        .route("/git/branch/create", post(handlers::git_branch_create))
        .route("/git/branch/delete", post(handlers::git_branch_delete))
        .route("/git/branch/rename", post(handlers::git_branch_rename))
        // Remotes
        .route("/git/sync-status", get(handlers::git_sync_status))
        .route("/git/fetch", post(handlers::git_fetch))
        .route("/git/push", post(handlers::git_push))
        // Worktrees
        .route("/git/worktrees", get(handlers::git_worktrees))
        .route("/git/worktree/add", post(handlers::git_worktree_add))
        .route("/git/worktree/remove", post(handlers::git_worktree_remove))
        .route("/git/worktree/prune", post(handlers::git_worktree_prune))
        // History rewriting and integration
        .route(
            "/git/operation",
            get(handlers::git_operation_status).post(handlers::git_operation),
        )
        .route("/git/merge", post(handlers::git_merge))
        .route("/git/rebase", post(handlers::git_rebase))
        .route("/git/reset", post(handlers::git_reset))
        .route("/git/revert", post(handlers::git_revert))
        .route("/git/cherry-pick", post(handlers::git_cherry_pick))
        // Tags and attribution
        .route("/git/tags", get(handlers::git_tags))
        .route("/git/tag", post(handlers::git_tag_create))
        .route("/git/tag/delete", post(handlers::git_tag_delete))
        .route("/git/blame", get(handlers::git_blame))
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
        // Every SSE stream of one frontend instance, multiplexed over one socket
        .route("/stream/ws", get(super::super::stream_mux::stream_ws))
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
            "/acp/agents/{id}/native-tools",
            post(handlers::set_native_tools),
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
