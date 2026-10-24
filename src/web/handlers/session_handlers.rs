//! Session message, command, provider, and permission/question handlers.

#[cfg(test)]
use super::super::auth::AuthUser;
#[cfg(test)]
use super::super::error::WebError;
#[cfg(test)]
use super::super::types::*;
#[cfg(test)]
use super::common::resolve_project_dir;
#[cfg(test)]
use crate::api::interactions::CommandError;
#[cfg(test)]
use crate::runner::RunnerKind;
#[cfg(test)]
use axum::extract::{Query, State};
#[cfg(test)]
use axum::http::StatusCode;
#[cfg(test)]
use axum::response::{IntoResponse, Json};

#[path = "session_catalog.rs"]
mod session_catalog;
#[path = "session_interactions.rs"]
mod session_interactions;
#[path = "session_message_read.rs"]
mod session_message_read;
#[path = "session_message_send.rs"]
mod session_message_send;
#[path = "session_session_ops.rs"]
mod session_session_ops;

pub(crate) use session_catalog::map_command_error;
pub use session_catalog::{execute_command, get_commands, get_providers};
pub(crate) use session_interactions::clear_asked_questions;
pub use session_interactions::{
    a2ui_callback, get_pending, mark_session_seen, reject_question, reply_permission,
    reply_question,
};
pub(crate) use session_interactions::{a2ui_callback_result, a2ui_callback_text};
pub use session_message_read::get_session_messages;
pub(crate) use session_message_read::paginate_messages;
pub(crate) use session_message_send::map_send_message_response;
pub use session_message_send::send_message;
pub use session_session_ops::{
    abort_session, clear_session_queue, delete_session, get_session_queue,
    remove_session_queue_item, rename_session,
};
pub(crate) use session_session_ops::{map_proxy_json_response, map_status_only_response};

#[cfg(test)]
#[path = "session_handlers_direct_tests.rs"]
mod session_handlers_direct_tests;

#[cfg(test)]
#[path = "session_handlers_a2ui_tests.rs"]
mod session_handlers_a2ui_tests;

#[cfg(test)]
#[path = "session_handlers_proxy_tests.rs"]
mod session_handlers_proxy_tests;

#[cfg(test)]
#[path = "session_handlers_maps_tests.rs"]
mod session_handlers_maps_tests;

#[cfg(test)]
#[path = "session_handlers_upstream_tests.rs"]
mod session_handlers_upstream_tests;

#[cfg(test)]
#[path = "session_handlers_commands_tests.rs"]
mod session_handlers_commands_tests;

#[cfg(test)]
#[path = "session_handlers_question_tests.rs"]
mod session_handlers_question_tests;
