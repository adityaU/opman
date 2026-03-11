//! Embedded frontend serving via `rust-embed`.
//!
//! The React build output (`web-ui/dist/`) is compiled into the binary.
//! All non-API routes fall through to this handler, which serves static
//! assets or returns `index.html` for SPA client-side routing.
//!
//! When an `instance_name` is configured (derived from `--tunnel-hostname`),
//! `/manifest.json` and `index.html` are patched at serve time so that
//! the PWA home-screen name and HTML title reflect the tunnel subdomain.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Response, StatusCode};
use axum::response::IntoResponse;
use rust_embed::Embed;

use super::types::ServerState;

#[derive(Embed)]
#[folder = "web-ui/dist"]
#[prefix = ""]
struct FrontendAssets;

/// Serve embedded frontend assets, falling back to `index.html` for SPA routes.
///
/// If the server has an `instance_name`, `/manifest.json` and `index.html` are
/// dynamically patched so the PWA install name and page title use that name.
pub async fn serve(State(state): State<ServerState>, uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Resolve theme background color once — used by manifest & index patches.
    let theme_bg: Option<String> = state.web_state.get_theme().await.map(|t| t.background);

    // ── Dynamic manifest.json ───────────────────────────────────────
    if path == "manifest.json" {
        if let Some(file) = FrontendAssets::get("manifest.json") {
            let mut json = String::from_utf8_lossy(&file.data).into_owned();

            // Patch app name if configured
            if let Some(ref name) = state.instance_name {
                json = json
                    .replace("\"name\": \"opman\"", &format!("\"name\": \"{}\"", name))
                    .replace("\"short_name\": \"opman\"", &format!("\"short_name\": \"{}\"", name));
            }

            // Patch background_color and theme_color to match the active theme
            // so the PWA splash screen and system chrome use the correct colour.
            if let Some(ref bg) = theme_bg {
                json = json
                    .replace(
                        "\"background_color\": \"#0a0a0a\"",
                        &format!("\"background_color\": \"{}\"", bg),
                    )
                    .replace(
                        "\"theme_color\": \"#0a0a0a\"",
                        &format!("\"theme_color\": \"{}\"", bg),
                    );
            }

            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/manifest+json")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(json))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
                .into_response();
        }
    }

    // ── Service worker — must be served with no-cache ─────────────
    // Browsers require the service worker script to be re-validated on
    // every update check.  Aggressive caching would prevent new versions
    // from being picked up.
    if path == "sw.js" {
        if let Some(file) = FrontendAssets::get("sw.js") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/javascript")
                .header(header::CACHE_CONTROL, "no-cache")
                .header("Service-Worker-Allowed", "/")
                .body(Body::from(file.data.to_vec()))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
                .into_response();
        }
    }

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

    // Fall back to index.html for SPA routing — inject instance name & theme if set
    if let Some(file) = FrontendAssets::get("index.html") {
        let mut html = String::from_utf8_lossy(&file.data).into_owned();

        // Patch title & iOS home-screen name if an instance name is set
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

        // Patch initial theme-color & html background to match the active theme
        // so the status-bar / gesture-pill area use the correct colour from
        // first paint — before JS applies the theme via syncMetaThemeColor().
        if let Some(ref bg) = theme_bg {
            html = html.replace(
                "<meta name=\"theme-color\" content=\"#0a0a0a\" />",
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            // Patch the inline CSS fallback: `var(--color-bg, #0a0a0a)` → `var(--color-bg, <bg>)`
            html = html.replace(
                "var(--color-bg, #0a0a0a)",
                &format!("var(--color-bg, {})", bg),
            );
        }

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(html))
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
