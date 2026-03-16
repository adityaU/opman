//! JS interop helpers — DOMPurify sanitization and Mermaid rendering.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Sanitize HTML using DOMPurify (loaded via CDN).
pub fn sanitize_html(content: &str) -> String {
    let window = web_sys::window().unwrap();
    let purify = js_sys::Reflect::get(&window, &JsValue::from_str("DOMPurify")).ok();
    match purify {
        Some(dp) if !dp.is_undefined() => {
            let sanitize_fn =
                js_sys::Reflect::get(&dp, &JsValue::from_str("sanitize")).unwrap_or(JsValue::NULL);
            if !sanitize_fn.is_function() {
                return content.to_string();
            }
            let func: js_sys::Function = sanitize_fn.unchecked_into();
            let result = func
                .call1(&dp, &JsValue::from_str(content))
                .unwrap_or(JsValue::from_str(content));
            result.as_string().unwrap_or_else(|| content.to_string())
        }
        _ => {
            log::warn!("DOMPurify not loaded — HTML not sanitized");
            content.to_string()
        }
    }
}

/// Sanitize SVG using DOMPurify with SVG profile (loaded via CDN).
pub fn sanitize_svg(content: &str) -> String {
    let window = web_sys::window().unwrap();
    let purify = js_sys::Reflect::get(&window, &JsValue::from_str("DOMPurify")).ok();
    match purify {
        Some(dp) if !dp.is_undefined() => {
            let sanitize_fn =
                js_sys::Reflect::get(&dp, &JsValue::from_str("sanitize")).unwrap_or(JsValue::NULL);
            if !sanitize_fn.is_function() {
                return content.to_string();
            }
            let func: js_sys::Function = sanitize_fn.unchecked_into();
            let config = js_sys::Object::new();
            let profiles = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &profiles,
                &JsValue::from_str("svg"),
                &JsValue::from_bool(true),
            );
            let _ = js_sys::Reflect::set(
                &profiles,
                &JsValue::from_str("svgFilters"),
                &JsValue::from_bool(true),
            );
            let _ = js_sys::Reflect::set(&config, &JsValue::from_str("USE_PROFILES"), &profiles);
            let result = func
                .call2(&dp, &JsValue::from_str(content), &config)
                .unwrap_or(JsValue::from_str(content));
            result.as_string().unwrap_or_else(|| content.to_string())
        }
        _ => {
            log::warn!("DOMPurify not loaded — SVG not sanitized");
            content.to_string()
        }
    }
}

/// Render mermaid diagram via mermaid.js CDN.
pub fn render_mermaid_js(
    content: &str,
    set_svg: WriteSignal<String>,
    set_error: WriteSignal<Option<String>>,
) {
    let window = web_sys::window().unwrap();
    let mermaid_js = js_sys::Reflect::get(&window, &JsValue::from_str("mermaid")).ok();
    let Some(m) = mermaid_js.filter(|v| !v.is_undefined()) else {
        set_error.set(Some("Mermaid library not loaded".to_string()));
        return;
    };

    // mermaid.initialize(...)
    let init_fn =
        js_sys::Reflect::get(&m, &JsValue::from_str("initialize")).unwrap_or(JsValue::NULL);
    if init_fn.is_function() {
        let func: js_sys::Function = init_fn.unchecked_into();
        let config = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &config,
            &JsValue::from_str("startOnLoad"),
            &JsValue::from_bool(false),
        );
        let _ = js_sys::Reflect::set(
            &config,
            &JsValue::from_str("theme"),
            &JsValue::from_str("base"),
        );
        let _ = js_sys::Reflect::set(
            &config,
            &JsValue::from_str("securityLevel"),
            &JsValue::from_str("strict"),
        );
        let _ = func.call1(&m, &config);
    }

    let id = format!("mermaid-{}", js_sys::Math::random().to_bits() & 0xFFFFFFFF);
    let render_fn = js_sys::Reflect::get(&m, &JsValue::from_str("render")).unwrap_or(JsValue::NULL);
    if !render_fn.is_function() {
        set_error.set(Some("mermaid.render not available".to_string()));
        return;
    }

    let func: js_sys::Function = render_fn.unchecked_into();
    match func.call2(&m, &JsValue::from_str(&id), &JsValue::from_str(content)) {
        Ok(p) => {
            let promise: js_sys::Promise = p.unchecked_into();
            let on_success = Closure::once(move |result: JsValue| {
                let svg_val = js_sys::Reflect::get(&result, &JsValue::from_str("svg"))
                    .unwrap_or(JsValue::NULL);
                if let Some(svg_str) = svg_val.as_string() {
                    set_error.set(None);
                    set_svg.set(svg_str);
                } else {
                    set_error.set(Some("Failed to render Mermaid diagram".to_string()));
                }
            });
            let on_error = Closure::once(move |err: JsValue| {
                let msg = err
                    .as_string()
                    .or_else(|| {
                        js_sys::Reflect::get(&err, &JsValue::from_str("message"))
                            .ok()
                            .and_then(|v| v.as_string())
                    })
                    .unwrap_or_else(|| "Failed to render Mermaid diagram".to_string());
                set_error.set(Some(msg));
                set_svg.set(String::new());
            });
            let _ = promise.then2(&on_success, &on_error);
            on_success.forget();
            on_error.forget();
        }
        Err(e) => {
            let msg = e
                .as_string()
                .unwrap_or_else(|| "Mermaid render failed".to_string());
            set_error.set(Some(msg));
        }
    }
}
