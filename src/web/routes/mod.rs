use axum::extract::DefaultBodyLimit;
use axum::Router;
use tower_http::compression::CompressionLayer;

use super::request_log;
use super::static_files;
use super::types::ServerState;

mod api;
mod internal;

pub(super) fn build_router(state: ServerState) -> Router {
    let api_routes = api::api_routes();
    let internal_routes = internal::internal_routes();

    // Public (unauthenticated) API routes — outside the main api_routes
    // so they don't go through the auth extractor.
    let public_routes: Router<ServerState> = Router::new().route(
        "/public/bootstrap",
        axum::routing::get(super::handlers::public_bootstrap),
    );

    Router::new()
        .route("/health", axum::routing::get(super::handlers::health))
        .nest("/api", public_routes.merge(api_routes))
        .nest("/internal", internal_routes)
        .fallback(static_files::serve_react)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50 MB global body limit
        .layer(CompressionLayer::new().gzip(true))
        // Outermost, so it also logs body-limit/auth rejections and sees the
        // request future get dropped when a client disconnects mid-request.
        .layer(axum::middleware::from_fn(request_log::log_requests))
        .with_state(state)
}

#[cfg(test)]
#[path = "../routes_tests.rs"]
mod routes_tests;
