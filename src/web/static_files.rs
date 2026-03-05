//! Embedded frontend serving via `rust-embed`.
//!
//! The React build output (`web-ui/dist/`) is compiled into the binary.
//! All non-API routes fall through to this handler, which serves static
//! assets or returns `index.html` for SPA client-side routing.

use axum::body::Body;
use axum::http::{header, Response, StatusCode};
use axum::response::IntoResponse;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web-ui/dist"]
#[prefix = ""]
struct FrontendAssets;

/// Serve embedded frontend assets, falling back to `index.html` for SPA routes.
pub async fn serve(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first
    if let Some(file) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(Body::from(file.data.to_vec()))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            })
            .into_response();
    }

    // Fall back to index.html for SPA routing
    if let Some(file) = FrontendAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(file.data.to_vec()))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            })
            .into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
