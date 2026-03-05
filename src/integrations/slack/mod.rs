//! Slack integration for opman.
//!
//! Provides self-chat DM messaging via Slack Socket Mode (WebSocket) with:
//! - OAuth 2.0 authentication flow (temporary local HTTP server for callback)
//! - AI-powered project detection (triage session)
//! - Session routing to free sessions in detected project
//! - Response batching and relay back to Slack threads
//! - Thread reply handling via system message injection
//!
//! Credentials stored in `~/.config/opman/slack_auth.yaml` (separate from config.toml).

pub mod api;
pub mod auth;
pub mod commands;
pub mod formatting;
pub mod relay;
pub mod socket;
pub mod state;
pub mod tools;
pub mod triage;
pub mod types;

// ── Re-exports ──────────────────────────────────────────────────────────
// Flat re-exports so that existing `crate::slack::Foo` paths continue to work.

// auth
pub use auth::run_oauth_flow;
pub use auth::{SlackAuth, SlackSessionMap, SlackSettings};

// types
pub use types::{
    SessionMeta, SlackBackgroundEvent, SlackConnectionStatus, SlackLogEntry, SlackLogLevel,
    SlackMetrics,
};

// state
pub use state::{slack_thread_link, SlackState};

// api
pub use api::{
    chunk_for_slack, fetch_all_session_messages, find_free_session, post_message,
    post_message_with_blocks, send_system_message, send_user_message, update_message,
    update_message_blocks,
};

// tools
pub use tools::fetch_session_messages_with_tools;

// formatting
pub use formatting::{
    render_permission_blocks, render_permission_confirmed_blocks, render_question_blocks,
    render_question_confirmed_blocks, render_question_dismissed_blocks, render_todos_mrkdwn,
};

// socket
pub use socket::spawn_socket_mode;

// triage
pub use triage::{build_triage_prompt, parse_triage_response, triage_project_dir};

// commands
pub use commands::{
    handle_list_projects_command, handle_session_command, handle_sessions_slash,
    handle_slash_command, handle_thread_slash_command, open_connect_session_modal,
    SlashCommandOutcome,
};

// relay
pub use relay::{spawn_response_batcher, spawn_session_relay_watcher};
