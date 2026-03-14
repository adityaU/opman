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

    // Resolve theme colours once — used by manifest, favicon & index patches.
    let theme_colors = state.web_state.get_theme().await;
    let theme_bg: Option<String> = theme_colors.as_ref().map(|t| t.background.clone());
    let theme_primary: Option<String> = theme_colors.as_ref().map(|t| t.primary.clone());

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

            // Patch background_color, theme_color, and inline SVG colours
            // to match the active theme so the PWA splash screen, system
            // chrome, and SVG favicon all use the correct colours.
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

    // ── Dynamic favicon.svg — patched with theme colours ───────────
    // The static favicon.svg uses hardcoded colours.  When a theme is
    // active, we swap in the theme's primary + background so the icon
    // matches everywhere (notification icon, bookmarks, etc.).
    if path == "favicon.svg" {
        if let (Some(ref primary), Some(ref bg)) = (&theme_primary, &theme_bg) {
            if let Some(file) = FrontendAssets::get("favicon.svg") {
                let mut svg = String::from_utf8_lossy(&file.data).into_owned();
                // Replace the hardcoded fill and stroke colours
                svg = svg
                    .replace("fill=\"#0a0a0a\"", &format!("fill=\"{}\"", bg))
                    .replace("fill=\"#0B0E14\"", &format!("fill=\"{}\"", bg))
                    .replace("stroke=\"#fab283\"", &format!("stroke=\"{}\"", primary));
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/svg+xml")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(Body::from(svg))
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::empty())
                            .unwrap()
                    })
                    .into_response();
            }
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
            // Replace all theme-color meta tags (may use #0B0E14 or #0a0a0a depending on build)
            html = html.replace(
                "<meta name=\"theme-color\" content=\"#0B0E14\" />",
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            html = html.replace(
                "<meta name=\"theme-color\" content=\"#0a0a0a\" />",
                &format!("<meta name=\"theme-color\" content=\"{}\" />", bg),
            );
            // Patch the inline CSS fallback: `var(--color-bg, #0B0E14)` → `var(--color-bg, <bg>)`
            html = html.replace(
                "var(--color-bg, #0B0E14)",
                &format!("var(--color-bg, {})", bg),
            );
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
