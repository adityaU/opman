//! Dynamic themed favicon, PWA manifest, and service-worker notification.
//!
//! The service worker intercepts `/favicon.svg`, `/icon-192.png`,
//! `/icon-512.png`, and `/manifest.json` to serve themed versions.
//! This module:
//!  1. Updates the in-page `<link rel="icon">` with a themed SVG blob.
//!  2. Replaces `<link rel="manifest">` with a blob manifest that has
//!     themed colors but keeps real `/icon-*.png` paths (SW intercepts them).
//!  3. Posts THEME_COLORS to the SW so it persists and serves themed icons.

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static PREV_MANIFEST_URL: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Build the themed SVG string for the terminal-chevron app icon.
fn build_theme_svg(primary: &str, bg: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="7" fill="{bg}"/>
  <path d="M7 22 L14 16 L7 10" stroke="{primary}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
  <line x1="16" y1="22" x2="25" y2="22" stroke="{primary}" stroke-width="2.5" stroke-linecap="round"/>
</svg>"#
    )
}

fn make_blob_url(content: &str, mime: &str) -> Option<String> {
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

/// Update `<link rel="icon">` to a themed SVG favicon.
pub fn update_favicon(primary: &str, bg: &str) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let svg = build_theme_svg(primary, bg);
    let Some(url) = make_blob_url(&svg, "image/svg+xml") else {
        return;
    };
    if let Ok(Some(el)) = doc.query_selector(r#"link[rel="icon"]"#) {
        let link = el.unchecked_ref::<web_sys::HtmlLinkElement>();
        let prev = link.href();
        if prev.starts_with("blob:") {
            let _ = web_sys::Url::revoke_object_url(&prev);
        }
        link.set_href(&url);
    } else if let Some(head) = doc.head() {
        if let Ok(el) = doc.create_element("link") {
            let _ = el.set_attribute("rel", "icon");
            let _ = el.set_attribute("type", "image/svg+xml");
            let _ = el.set_attribute("href", &url);
            let _ = head.append_child(&el);
        }
    }
}

/// Update `<link rel="manifest">` with themed colors but real icon paths.
///
/// Android Chrome fetches manifest icon URLs in its own process, so they
/// must be real paths (not blob:). The service worker intercepts
/// `/icon-192.png` and `/icon-512.png` to return themed PNGs.
pub fn update_pwa_icons(_primary: &str, bg: &str) {
    let bg = bg.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        update_manifest_inner(&bg).await;
    });
}

async fn update_manifest_inner(bg: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(doc) = window.document() else { return };

    let existing = doc.query_selector(r#"link[rel="manifest"]"#).ok().flatten();
    let href = existing
        .as_ref()
        .map(|e| e.unchecked_ref::<web_sys::HtmlLinkElement>().href())
        .unwrap_or_else(|| "/manifest.json".to_string());

    // Fetch to get the base manifest (may already be themed if SW active)
    let fetch_url = if href.starts_with("blob:") {
        "/manifest.json".to_string()
    } else {
        href
    };
    let Ok(resp_val) =
        wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&fetch_url)).await
    else {
        return;
    };
    let Ok(resp) = resp_val.dyn_into::<web_sys::Response>() else {
        return;
    };
    if !resp.ok() {
        return;
    }
    let Ok(json_promise) = resp.json() else { return };
    let Ok(manifest) = wasm_bindgen_futures::JsFuture::from(json_promise).await else {
        return;
    };

    // Patch icons to use real paths (SW intercepts these)
    let icons = js_sys::Array::new();
    for (src, sz, purpose) in [
        ("/favicon.svg", "any", "any"),
        ("/icon-192.png", "192x192", "any"),
        ("/icon-512.png", "512x512", "any"),
        ("/icon-192.png", "192x192", "maskable"),
        ("/icon-512.png", "512x512", "maskable"),
    ] {
        let o = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&o, &"src".into(), &JsValue::from_str(src));
        let _ = js_sys::Reflect::set(&o, &"sizes".into(), &JsValue::from_str(sz));
        let mime = if src.ends_with(".svg") {
            "image/svg+xml"
        } else {
            "image/png"
        };
        let _ = js_sys::Reflect::set(&o, &"type".into(), &JsValue::from_str(mime));
        let _ = js_sys::Reflect::set(&o, &"purpose".into(), &JsValue::from_str(purpose));
        icons.push(&o);
    }
    let _ = js_sys::Reflect::set(&manifest, &"icons".into(), &icons);
    let _ = js_sys::Reflect::set(&manifest, &"theme_color".into(), &JsValue::from_str(bg));
    let _ = js_sys::Reflect::set(&manifest, &"background_color".into(), &JsValue::from_str(bg));

    let Ok(s) = js_sys::JSON::stringify(&manifest) else { return };
    let json_string: String = s.into();
    let Some(new_url) = make_blob_url(&json_string, "application/manifest+json") else {
        return;
    };

    // Revoke previous manifest blob
    PREV_MANIFEST_URL.with(|c| {
        if let Some(ref old) = *c.borrow() {
            let _ = web_sys::Url::revoke_object_url(old);
        }
    });

    if let Some(el) = existing {
        el.unchecked_ref::<web_sys::HtmlLinkElement>()
            .set_href(&new_url);
    } else if let Some(head) = doc.head() {
        if let Ok(el) = doc.create_element("link") {
            let _ = el.set_attribute("rel", "manifest");
            let _ = el.set_attribute("href", &new_url);
            let _ = head.append_child(&el);
        }
    }
    PREV_MANIFEST_URL.with(|c| *c.borrow_mut() = Some(new_url));
}

/// Post theme colors to the active service worker.
pub fn notify_service_worker(primary: &str, bg: &str) {
    let Some(window) = web_sys::window() else { return };
    let sw = window.navigator().service_worker();
    if let Some(controller) = sw.controller() {
        post_theme_msg(&controller, primary, bg);
    } else {
        let p = primary.to_string();
        let b = bg.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            notify_sw_ready(&p, &b).await;
        });
    }
}

fn post_theme_msg(sw: &web_sys::ServiceWorker, primary: &str, bg: &str) {
    let msg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&msg, &"type".into(), &"THEME_COLORS".into());
    let colors = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&colors, &"primary".into(), &JsValue::from_str(primary));
    let _ = js_sys::Reflect::set(&colors, &"background".into(), &JsValue::from_str(bg));
    let _ = js_sys::Reflect::set(&msg, &"colors".into(), &colors);
    let _ = sw.post_message(&msg);
}

async fn notify_sw_ready(primary: &str, bg: &str) {
    let Some(window) = web_sys::window() else { return };
    let Ok(promise) = window.navigator().service_worker().ready() else {
        return;
    };
    let Ok(val) = wasm_bindgen_futures::JsFuture::from(promise).await else {
        return;
    };
    if let Ok(reg) = val.dyn_into::<web_sys::ServiceWorkerRegistration>() {
        if let Some(active) = reg.active() {
            post_theme_msg(&active, primary, bg);
        }
    }
}
