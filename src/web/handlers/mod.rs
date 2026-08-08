//! REST API route handlers.
//!
//! Authentication is enforced via the `AuthUser` extractor — handlers that
//! include it in their signature automatically reject unauthenticated requests.
//!
//! State queries use the independent `WebStateHandle` (no TUI dependency).
//! Terminal I/O goes directly to the `WebPtyManager` (independent web PTYs).

mod acp_handlers;
mod acp_upsert;
mod agents_handlers;
mod auth_handlers;
mod common;
mod context_handlers;
mod doc_handlers;
mod doc_readers;
mod doc_readers_docx;
mod doc_writers;
mod doc_writers_html;
mod download_handlers;
mod editor_handlers;
mod files_handlers;
mod git_context_handlers;
mod git_ext_handlers;
mod git_handlers;
mod health_handlers;
mod kanban_handlers;
mod kanban_internal;
mod kanban_internal_query;
mod keybindings_handlers;
mod mcp_handlers;
mod mcp_login;
mod mcp_login_state;
mod mcp_tools_handlers;
mod mcp_upsert;
mod memory_handlers;
mod presence_handlers;
mod project_handlers;
mod pty_handlers;
mod routines_handlers;
mod search_handlers;
mod session_handlers;
mod skills_handlers;
mod state_handlers;
pub(crate) mod system_handlers;
mod watcher_handlers;

#[cfg(test)]
#[path = "tests.rs"]
mod handler_tests;

// ── Re-exports ──────────────────────────────────────────────────────
// All public handler functions are re-exported so that `handlers::foo`
// continues to work from the router in `super::mod`.

pub use auth_handlers::{health, login, verify};
pub use keybindings_handlers::{get_keybindings, put_keybindings};

pub use state_handlers::{
    get_session_stats, get_state, get_theme, list_themes, public_bootstrap, switch_theme,
};

pub use project_handlers::{
    add_project, browse_dirs, focus_panel, home_dir, new_session, remove_project, select_session,
    switch_project, toggle_panel,
};

pub use pty_handlers::{pty_activity, pty_kill, pty_list, pty_resize, pty_write, spawn_pty};

pub use session_handlers::{
    a2ui_callback, abort_session, clear_session_queue, delete_session, execute_command,
    get_commands, get_pending, get_providers, get_session_messages, get_session_queue,
    mark_session_seen, remove_session_queue_item, rename_session, reply_permission, reply_question,
    send_message,
};

pub use git_handlers::{
    git_commit, git_diff, git_discard, git_log, git_stage, git_status, git_unstage,
};

pub use git_ext_handlers::{
    git_branches, git_checkout, git_gitignore, git_pull, git_range_diff, git_show, git_stash,
};

pub use git_context_handlers::{git_context_summary, git_repos};

pub use agents_handlers::get_agents;

pub use files_handlers::{
    browse_files, create_dir, create_file, delete_dir, delete_file, read_file, read_file_raw,
    rename_entry, search_files, upload_files, write_file,
};

pub use doc_handlers::{doc_read, doc_write};

pub use download_handlers::{download_dir, download_file};

pub use editor_handlers::{
    editor_lsp_completion, editor_lsp_definition, editor_lsp_diagnostics, editor_lsp_format,
    editor_lsp_hover,
};

pub use context_handlers::{get_context_window, get_session_todos, update_session_todos};

pub use search_handlers::{get_file_edits, search_messages};

pub use watcher_handlers::{
    create_watcher, delete_watcher, get_watcher, get_watcher_messages, get_watcher_sessions,
    list_watchers,
};

pub use memory_handlers::{
    create_personal_memory, delete_personal_memory, get_autonomy_settings, list_active_memory,
    list_personal_memory, update_autonomy_settings, update_personal_memory,
};

pub use presence_handlers::{deregister_presence, get_presence, register_presence};

pub use routines_handlers::{
    create_routine, delete_routine, list_routines, run_routine, update_routine,
};

pub use system_handlers::get_system_stats;

pub use acp_handlers::{delete_agent, list_agents, set_agent_enabled};
pub use acp_upsert::upsert_agent;
pub use mcp_handlers::{delete_server, list_servers, set_enabled};
pub use mcp_login::{finish_login, logout_server, start_login};
pub use mcp_login_state::LoginSessions;
pub use mcp_tools_handlers::list_tools;
pub use mcp_upsert::upsert_server;
pub use skills_handlers::{
    create_skill, delete_skill, get_skill, list_skills, update_skill, upload_skills,
};

pub use health_handlers::{
    get_health_audit, get_health_status, set_health_config, toggle_health_mitigation,
};

pub use kanban_handlers::{
    abort_task, add_user_note, create_task, delete_task, get_board, get_task, launch_task,
    serve_asset, update_board_config, update_task, upload_attachment,
};

pub use kanban_internal::{
    internal_add_note, internal_complete, internal_get_task, internal_set_status,
};

pub use kanban_internal_query::{
    internal_board_overview, internal_query_tasks, internal_read_notes,
};
