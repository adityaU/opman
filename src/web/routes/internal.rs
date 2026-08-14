use axum::routing::{get, post};
use axum::Router;

use super::super::handlers;

pub(super) fn internal_routes() -> Router<super::super::types::ServerState> {
    // Internal loopback API for opman's own stdio MCP servers. It uses the
    // shared token checked by the handlers, not the browser AuthUser extractor.
    Router::new()
        .route("/ask", post(handlers::internal_ask))
        .route("/browser", post(handlers::internal_browser))
        .route("/kanban/task/{task_id}", get(handlers::internal_get_task))
        .route(
            "/kanban/task/{task_id}/status",
            post(handlers::internal_set_status),
        )
        .route(
            "/kanban/task/{task_id}/note",
            post(handlers::internal_add_note),
        )
        .route(
            "/kanban/task/{task_id}/complete",
            post(handlers::internal_complete),
        )
        .route(
            "/kanban/task/{task_id}/query",
            post(handlers::internal_query_tasks),
        )
        .route(
            "/kanban/task/{task_id}/board",
            get(handlers::internal_board_overview),
        )
        .route(
            "/kanban/task/{task_id}/notes",
            post(handlers::internal_read_notes),
        )
}
