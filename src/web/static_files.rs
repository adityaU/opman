//! Embedded frontend serving via `rust-embed`.
//!
//! React (`web-ui/dist/`) serves at `/`.
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
#[folder = "web-ui/dist"]
#[prefix = ""]
struct ReactAssets;

/// Hardcoded fallback colours used in the React UI source files.
struct UiThemeDefaults {
    bg: &'static str,
    sw_allowed: &'static str,
}

const REACT_DEFAULTS: UiThemeDefaults = UiThemeDefaults {
    bg: "#0B0E14",
    sw_allowed: "/",
};

/// Pre-resolved theme values passed into the sync serving helper.
struct ResolvedTheme {
    bg: Option<String>,
    primary: Option<String>,
}

/// Shared serving logic for an embedded UI.
///
/// `get_file` resolves an asset path to its bytes+metadata.
/// `defaults` carries the hardcoded theme colours for manifest/index patching.
fn serve_ui<F>(
    state: &ServerState,
    headers: &HeaderMap,
    path: &str,
    get_file: F,
    defaults: &UiThemeDefaults,
    theme: &ResolvedTheme,
) -> axum::response::Response
where
    F: Fn(&str) -> Option<rust_embed::EmbeddedFile>,
{
    // ── Dynamic manifest.json ───────────────────────────────────────
    if path == "manifest.json" {
        if let Some(file) = get_file("manifest.json") {
            let mut json = String::from_utf8_lossy(&file.data).into_owned();

            if let Some(ref name) = state.instance_name {
                json = json
                    .replace("\"name\": \"opman\"", &format!("\"name\": \"{}\"", name))
                    .replace(
                        "\"short_name\": \"opman\"",
                        &format!("\"short_name\": \"{}\"", name),
                    );
            }

            if let Some(ref bg) = theme.bg {
                json = json
                    .replace(
                        &format!("\"background_color\": \"{}\"", defaults.bg),
                        &format!("\"background_color\": \"{}\"", bg),
                    )
                    .replace(
                        &format!("\"theme_color\": \"{}\"", defaults.bg),
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
        if let Some(file) = get_file("sw.js") {
            let r = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/javascript")
                .header(header::CACHE_CONTROL, "no-cache")
                .header("Service-Worker-Allowed", defaults.sw_allowed);
            return build_ok(r, Body::from(file.data.to_vec())).into_response();
        }
    }

    // ── Dynamic favicon.svg — patched with theme colours ───────────
    if path == "favicon.svg" {
        if let (Some(ref primary), Some(ref bg)) = (&theme.primary, &theme.bg) {
            if let Some(file) = get_file("favicon.svg") {
                let mut svg = String::from_utf8_lossy(&file.data).into_owned();
                svg = svg
                    .replace(
                        &format!("fill=\"{}\"", defaults.bg),
                        &format!("fill=\"{}\"", bg),
                    )
                    .replace(
                        "stroke=\"#fab283\"",
                        &format!("stroke=\"{}\"", primary),
                    );
                let r = Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/svg+xml")
                    .header(header::CACHE_CONTROL, "no-cache");
                return build_ok(r, Body::from(svg)).into_response();
            }
        }
    }

    // ── Static asset with ETag ────────────────────────────────────
    if let Some(file) = get_file(path) {
        let etag = etag_from_hash(&file.metadata.sha256_hash());
        if is_not_modified(headers, &etag) {
            return build_ok(
                Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, &etag),
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

    // ── Fall back to index.html for SPA routing ───────────────────
    if let Some(file) = get_file("index.html") {
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

        if let Some(ref bg) = theme.bg {
            html = html.replace(
                &format!("<meta name=\"theme-color\" content=\"{}\" />", defaults.bg),
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            html = html.replace(
                &format!("var(--color-bg, {})", defaults.bg),
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

/// Resolve theme values from the async web state into a sync-friendly struct.
async fn resolve_theme(state: &ServerState) -> ResolvedTheme {
    let theme_pair = state.web_state.get_theme().await;
    ResolvedTheme {
        bg: theme_pair.as_ref().map(|t| t.dark.background.clone()),
        primary: theme_pair.as_ref().map(|t| t.dark.primary.clone()),
    }
}

/// Serve React UI at `/` — used as the top-level router fallback.
pub async fn serve_react(
    State(state): State<ServerState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let theme = resolve_theme(&state).await;
    serve_ui(&state, &headers, path, |p| ReactAssets::get(p), &REACT_DEFAULTS, &theme)
}
