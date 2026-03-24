//! Embedded frontend serving via `rust-embed`.
//!
//! Leptos (`leptos-ui/dist/`) serves at `/`; React (`web-ui/dist/`) at `/ui`.
//! When `instance_name` is set, manifest/index are patched for PWA naming.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::response::IntoResponse;
use rust_embed::Embed;

use super::types::ServerState;

/// Build a hex ETag string from the rust-embed SHA-256 hash (first 16 bytes).
fn etag_from_hash(hash: &[u8; 32]) -> String {
    let hex: String = hash[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

/// Check if the request's `If-None-Match` header matches the ETag.
fn is_not_modified(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v.contains(etag))
}

/// Build a response, falling back to 500 on builder error.
fn build_ok(builder: axum::http::response::Builder, body: Body) -> Response<Body> {
    builder.body(body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
    })
}

#[derive(Embed)]
#[folder = "leptos-ui/dist"]
#[prefix = ""]
struct FrontendAssets;

// ── React UI (served at /ui) ────────────────────────────────────────

#[derive(Embed)]
#[folder = "web-ui/dist"]
#[prefix = ""]
struct ReactAssets;

/// Serve the React frontend at `/ui`.
///
/// Requests to `/ui` or `/ui/anything` that match an embedded asset file
/// are served directly.  All other paths fall back to `index.html` for
/// React client-side routing.
pub async fn serve_react(headers: HeaderMap, uri: axum::http::Uri) -> impl IntoResponse {
    let full = uri.path();
    let path = full
        .strip_prefix("/ui/")
        .or_else(|| full.strip_prefix("/ui"))
        .unwrap_or("");
    let path = if path.is_empty() { "index.html" } else { path };

    if path == "sw.js" {
        if let Some(file) = ReactAssets::get("sw.js") {
            let r = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/javascript")
                .header(header::CACHE_CONTROL, "no-cache")
                .header("Service-Worker-Allowed", "/ui/");
            return build_ok(r, Body::from(file.data.to_vec())).into_response();
        }
    }

    if let Some(file) = ReactAssets::get(path) {
        let etag = etag_from_hash(&file.metadata.sha256_hash());
        if is_not_modified(&headers, &etag) {
            return build_ok(
                Response::builder().status(StatusCode::NOT_MODIFIED).header(header::ETAG, &etag),
                Body::empty(),
            )
            .into_response();
        }
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache = if path == "index.html" {
            "no-cache"
        } else {
            "public, max-age=31536000, immutable"
        };
        let r = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache)
            .header(header::ETAG, &etag);
        return build_ok(r, Body::from(file.data.to_vec())).into_response();
    }

    // Fall back to index.html for SPA routing
    if let Some(file) = ReactAssets::get("index.html") {
        let r = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CACHE_CONTROL, "no-cache");
        return build_ok(r, Body::from(file.data.to_vec())).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

/// Serve embedded Leptos frontend assets at `/`, falling back to `index.html`
/// for SPA routes.
///
/// If the server has an `instance_name`, `/manifest.json` and `index.html` are
/// dynamically patched so the PWA install name and page title use that name.
pub async fn serve(State(state): State<ServerState>, headers: HeaderMap, uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Resolve theme colours once — used by manifest, favicon & index patches.
    let theme_pair = state.web_state.get_theme().await;
    let theme_bg: Option<String> = theme_pair.as_ref().map(|t| t.dark.background.clone());
    let theme_primary: Option<String> = theme_pair.as_ref().map(|t| t.dark.primary.clone());

    // ── Dynamic manifest.json ───────────────────────────────────────
    if path == "manifest.json" {
        if let Some(file) = FrontendAssets::get("manifest.json") {
            let mut json = String::from_utf8_lossy(&file.data).into_owned();

            if let Some(ref name) = state.instance_name {
                json = json
                    .replace("\"name\": \"opman\"", &format!("\"name\": \"{}\"", name))
                    .replace(
                        "\"short_name\": \"opman\"",
                        &format!("\"short_name\": \"{}\"", name),
                    );
            }

            if let Some(ref bg) = theme_bg {
                json = json
                    .replace(
                        "\"background_color\": \"#0B0E14\"",
                        &format!("\"background_color\": \"{}\"", bg),
                    )
                    .replace(
                        "\"background_color\": \"#0a0a0a\"",
                        &format!("\"background_color\": \"{}\"", bg),
                    )
                    .replace(
                        "\"theme_color\": \"#0B0E14\"",
                        &format!("\"theme_color\": \"{}\"", bg),
                    )
                    .replace(
                        "\"theme_color\": \"#0a0a0a\"",
                        &format!("\"theme_color\": \"{}\"", bg),
                    );
            }

            let r = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/manifest+json")
                .header(header::CACHE_CONTROL, "no-cache");
            return build_ok(r, Body::from(json)).into_response();
        }
    }

    // ── Service worker — must be served with no-cache ─────────────
    if path == "sw.js" {
        if let Some(file) = FrontendAssets::get("sw.js") {
            let r = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/javascript")
                .header(header::CACHE_CONTROL, "no-cache")
                .header("Service-Worker-Allowed", "/");
            return build_ok(r, Body::from(file.data.to_vec())).into_response();
        }
    }

    // ── Dynamic favicon.svg — patched with theme colours ───────────
    if path == "favicon.svg" {
        if let (Some(ref primary), Some(ref bg)) = (&theme_primary, &theme_bg) {
            if let Some(file) = FrontendAssets::get("favicon.svg") {
                let mut svg = String::from_utf8_lossy(&file.data).into_owned();
                svg = svg
                    .replace("fill=\"#0a0a0a\"", &format!("fill=\"{}\"", bg))
                    .replace("fill=\"#0B0E14\"", &format!("fill=\"{}\"", bg))
                    .replace("stroke=\"#fab283\"", &format!("stroke=\"{}\"", primary));
                let r = Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/svg+xml")
                    .header(header::CACHE_CONTROL, "no-cache");
                return build_ok(r, Body::from(svg)).into_response();
            }
        }
    }

    if let Some(file) = FrontendAssets::get(path) {
        let etag = etag_from_hash(&file.metadata.sha256_hash());
        if is_not_modified(&headers, &etag) {
            return build_ok(
                Response::builder().status(StatusCode::NOT_MODIFIED).header(header::ETAG, &etag),
                Body::empty(),
            )
            .into_response();
        }
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let r = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .header(header::ETAG, &etag);
        return build_ok(r, Body::from(file.data.to_vec())).into_response();
    }

    // Fall back to index.html for SPA routing — inject instance name & theme
    if let Some(file) = FrontendAssets::get("index.html") {
        let mut html = String::from_utf8_lossy(&file.data).into_owned();

        if let Some(ref name) = state.instance_name {
            html = html.replace("<title>opman</title>", &format!("<title>{}</title>", name));
            html = html.replace(
                "<meta name=\"apple-mobile-web-app-status-bar-style\" content=\"black-translucent\" />",
                &format!(
                    "<meta name=\"apple-mobile-web-app-status-bar-style\" content=\"black-translucent\" />\n    <meta name=\"apple-mobile-web-app-title\" content=\"{}\" />",
                    name
                ),
            );
        }

        if let Some(ref bg) = theme_bg {
            html = html.replace(
                "<meta name=\"theme-color\" content=\"#0B0E14\" />",
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            html = html.replace(
                "<meta name=\"theme-color\" content=\"#0a0a0a\" />",
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            html = html.replace(
                "var(--color-bg, #0B0E14)",
                &format!("var(--color-bg, {})", bg),
            );
            html = html.replace(
                "var(--color-bg, #0a0a0a)",
                &format!("var(--color-bg, {})", bg),
            );
        }

        let r = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CACHE_CONTROL, "no-cache");
        return build_ok(r, Body::from(html)).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
