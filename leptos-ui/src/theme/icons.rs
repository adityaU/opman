//! Dynamic themed favicon, PWA icons, and service-worker notification.
//! Mirrors React `utils/theme.ts` updateFavicon / updatePwaIcons /
//! notifyServiceWorker using web_sys + js_sys interop.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static PREV_APPLE_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    static PREV_MANIFEST_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    static PREV_192_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    static PREV_512_URL: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn revoke_prev(cell: &'static std::thread::LocalKey<RefCell<Option<String>>>) {
    cell.with(|c| {
        if let Some(ref url) = *c.borrow() {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    });
}

fn store_prev(cell: &'static std::thread::LocalKey<RefCell<Option<String>>>, url: String) {
    cell.with(|c| *c.borrow_mut() = Some(url));
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

fn svg_to_blob_url(svg: &str) -> Option<String> {
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(svg));
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.set_type("image/svg+xml");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
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
    let Some(url) = svg_to_blob_url(&build_theme_svg(primary, bg)) else {
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

/// Async: render SVG to PNG blob URL via Image + Canvas.
async fn svg_to_png_async(svg: &str, size: u32) -> Result<String, ()> {
    let doc = web_sys::window().and_then(|w| w.document()).ok_or(())?;
    let svg_url = svg_to_blob_url(svg).ok_or(())?;

    let img = web_sys::HtmlImageElement::new().map_err(|_| ())?;
    let (tx, rx) = futures::channel::oneshot::channel::<bool>();
    let tx: Rc<RefCell<Option<futures::channel::oneshot::Sender<bool>>>> =
        Rc::new(RefCell::new(Some(tx)));

    let tx_ok = Rc::clone(&tx);
    let on_load = Closure::once(move || {
        if let Some(t) = tx_ok.borrow_mut().take() {
            let _ = t.send(true);
        }
    });
    let tx_err = Rc::clone(&tx);
    let on_error = Closure::once(move || {
        if let Some(t) = tx_err.borrow_mut().take() {
            let _ = t.send(false);
        }
    });

    img.set_onload(Some(on_load.as_ref().unchecked_ref()));
    img.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    img.set_src(&svg_url);

    let loaded = rx.await.unwrap_or(false);
    let _ = web_sys::Url::revoke_object_url(&svg_url);
    if !loaded {
        return Err(());
    }

    let canvas = doc
        .create_element("canvas")
        .map_err(|_| ())?
        .unchecked_into::<web_sys::HtmlCanvasElement>();
    canvas.set_width(size);
    canvas.set_height(size);

    let ctx = canvas
        .get_context("2d")
        .map_err(|_| ())?
        .ok_or(())?
        .unchecked_into::<web_sys::CanvasRenderingContext2d>();

    ctx.draw_image_with_html_image_element_and_dw_and_dh(
        &img, 0.0, 0.0, size as f64, size as f64,
    )
    .map_err(|_| ())?;

    let (btx, brx) = futures::channel::oneshot::channel::<Option<web_sys::Blob>>();
    let btx: Rc<RefCell<Option<futures::channel::oneshot::Sender<Option<web_sys::Blob>>>>> =
        Rc::new(RefCell::new(Some(btx)));
    let cb = Closure::once(move |val: JsValue| {
        let b = val.dyn_into::<web_sys::Blob>().ok();
        if let Some(t) = btx.borrow_mut().take() {
            let _ = t.send(b);
        }
    });
    canvas.to_blob(cb.as_ref().unchecked_ref()).map_err(|_| ())?;

    let blob = brx.await.map_err(|_| ())?.ok_or(())?;
    web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| ())
}

/// Fire-and-forget: update apple-touch-icon + manifest with themed PNGs.
pub fn update_pwa_icons(primary: &str, bg: &str) {
    let primary = primary.to_string();
    let bg = bg.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = update_pwa_icons_inner(&primary, &bg).await;
    });
}

async fn update_pwa_icons_inner(primary: &str, bg: &str) -> Result<(), ()> {
    let svg = build_theme_svg(primary, bg);
    let (r192, r512) = futures::join!(svg_to_png_async(&svg, 192), svg_to_png_async(&svg, 512));
    let url192 = r192?;
    let url512 = r512.map_err(|_| {
        let _ = web_sys::Url::revoke_object_url(&url192);
    })?;

    let doc = web_sys::window().and_then(|w| w.document()).ok_or(())?;

    // Apple-touch-icon
    update_link(&doc, "apple-touch-icon", &url192, &PREV_APPLE_URL);

    // Dynamic manifest
    update_manifest(&doc, &url192, &url512, bg).await;

    revoke_prev(&PREV_192_URL);
    revoke_prev(&PREV_512_URL);
    store_prev(&PREV_192_URL, url192);
    store_prev(&PREV_512_URL, url512);
    Ok(())
}

fn update_link(
    doc: &web_sys::Document,
    rel: &str,
    href: &str,
    prev: &'static std::thread::LocalKey<RefCell<Option<String>>>,
) {
    let selector = format!(r#"link[rel="{rel}"]"#);
    if let Ok(Some(el)) = doc.query_selector(&selector) {
        revoke_prev(prev);
        el.unchecked_ref::<web_sys::HtmlLinkElement>().set_href(href);
    } else if let Some(head) = doc.head() {
        if let Ok(el) = doc.create_element("link") {
            let _ = el.set_attribute("rel", rel);
            let _ = el.set_attribute("href", href);
            let _ = head.append_child(&el);
        }
    }
    store_prev(prev, href.to_string());
}

async fn update_manifest(doc: &web_sys::Document, u192: &str, u512: &str, bg: &str) {
    let existing = doc.query_selector(r#"link[rel="manifest"]"#).ok().flatten();
    let href = existing
        .as_ref()
        .map(|e| e.unchecked_ref::<web_sys::HtmlLinkElement>().href())
        .unwrap_or_else(|| "/manifest.json".to_string());

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let Ok(resp_val) = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&href)).await
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

    // Patch icons array
    let icons = js_sys::Array::new();
    for (src, sz, purpose) in [
        (u192, "192x192", "any"),
        (u512, "512x512", "any"),
        (u192, "192x192", "maskable"),
        (u512, "512x512", "maskable"),
    ] {
        let o = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&o, &"src".into(), &JsValue::from_str(src));
        let _ = js_sys::Reflect::set(&o, &"sizes".into(), &JsValue::from_str(sz));
        let _ = js_sys::Reflect::set(&o, &"type".into(), &"image/png".into());
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

    if let Some(el) = existing {
        revoke_prev(&PREV_MANIFEST_URL);
        el.unchecked_ref::<web_sys::HtmlLinkElement>().set_href(&new_url);
    } else if let Some(head) = doc.head() {
        if let Ok(el) = doc.create_element("link") {
            let _ = el.set_attribute("rel", "manifest");
            let _ = el.set_attribute("href", &new_url);
            let _ = head.append_child(&el);
        }
    }
    store_prev(&PREV_MANIFEST_URL, new_url);
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
