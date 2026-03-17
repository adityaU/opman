//! REST API route handlers.
//!
//! Authentication is enforced via the `AuthUser` extractor — handlers that
//! include it in their signature automatically reject unauthenticated requests.
//!
//! State queries use the independent `WebStateHandle` (no TUI dependency).
//! Terminal I/O goes directly to the `WebPtyManager` (independent web PTYs).

mod common;
mod auth_handlers;
mod state_handlers;
mod project_handlers;
mod pty_handlers;
mod session_handlers;
mod git_handlers;
mod git_ext_handlers;
mod git_context_handlers;
mod agents_handlers;
mod files_handlers;
mod editor_handlers;
mod context_handlers;
mod search_handlers;
mod watcher_handlers;
mod dashboard_handlers;
mod dashboard_ext_handlers;
mod intelligence_handlers;
pub(crate) mod system_handlers;

#[cfg(test)]
#[path = "tests.rs"]
mod handler_tests;

// ── Re-exports ──────────────────────────────────────────────────────
// All public handler functions are re-exported so that `handlers::foo`
// continues to work from the router in `super::mod`.

pub use auth_handlers::{health, login, verify};

pub use state_handlers::{get_state, get_session_stats, get_theme, list_themes, switch_theme, public_bootstrap};

pub use project_handlers::{
    switch_project, select_session, new_session, add_project, remove_project,
    home_dir, browse_dirs, toggle_panel, focus_panel,
};

pub use pty_handlers::{spawn_pty, pty_write, pty_resize, pty_kill, pty_list};

pub use session_handlers::{
    get_session_messages, send_message, abort_session, delete_session, rename_session,
    execute_command, get_providers, get_commands, reply_permission, reply_question,
    get_pending,
};

pub use git_handlers::{
    git_status, git_diff, git_log, git_stage, git_unstage, git_commit, git_discard,
};

pub use git_ext_handlers::{git_show, git_branches, git_checkout, git_range_diff, git_pull, git_stash, git_gitignore};

pub use git_context_handlers::{git_context_summary, git_repos};

pub use agents_handlers::get_agents;

pub use files_handlers::{browse_files, read_file, read_file_raw, write_file, create_file, create_dir, delete_file, delete_dir, upload_files};

pub use editor_handlers::{
    editor_lsp_diagnostics, editor_lsp_hover, editor_lsp_definition, editor_lsp_format,
};

pub use context_handlers::{get_session_todos, update_session_todos, get_context_window};

pub use search_handlers::{get_file_edits, search_messages};

pub use watcher_handlers::{
    list_watchers, create_watcher, delete_watcher, get_watcher,
    get_watcher_sessions, get_watcher_messages,
};

pub use dashboard_handlers::{
    sessions_overview, sessions_tree, get_presence, register_presence, deregister_presence,
    get_activity_feed, list_missions, get_mission, create_mission, update_mission,
    delete_mission, mission_action,
    list_personal_memory, create_personal_memory, update_personal_memory, delete_personal_memory,
    get_autonomy_settings, update_autonomy_settings,
};

pub use dashboard_ext_handlers::{
    list_routines, create_routine, update_routine, delete_routine, run_routine,
    list_delegated_work, create_delegated_work, update_delegated_work, delete_delegated_work,
    list_workspaces, save_workspace, delete_workspace,
};

pub use system_handlers::get_system_stats;

pub use intelligence_handlers::{
    compute_inbox, compute_recommendations,
    compute_session_handoff, compute_resume_briefing, compute_daily_summary,
    list_signals, add_signal, compute_assistant_stats, list_workspace_templates,
    list_active_memory,
};
