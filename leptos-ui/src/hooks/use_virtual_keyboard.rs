//! Virtual keyboard detection hook — sets CSS/data attributes on <html>.
//! Matches React `useVirtualKeyboard.ts`.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Detect mobile virtual keyboard open/close via the Visual Viewport API.
/// Sets `data-vkb-open` attribute and `--vkb-height` CSS variable on `<html>`.
pub fn use_virtual_keyboard() {
    Effect::new(move |_| {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        // Check for Visual Viewport API
        let vv = js_sys::Reflect::get(&window, &JsValue::from_str("visualViewport"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null());
        let vv = match vv {
            Some(v) => v,
            None => return,
        };

        let inner_height = window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let doc_el = window.document().and_then(|d| d.document_element());

        let doc_el_resize = doc_el.clone();
        let vv_resize = vv.clone();

        let handler = Closure::<dyn Fn()>::new(move || {
            let viewport_height = js_sys::Reflect::get(&vv_resize, &JsValue::from_str("height"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(inner_height);

            let is_open = inner_height - viewport_height > 150.0;

            if let Some(ref el) = doc_el_resize {
                if is_open {
                    let _ = el.set_attribute("data-vkb-open", "true");
                } else {
                    let _ = el.remove_attribute("data-vkb-open");
                }
                let style = el.unchecked_ref::<web_sys::HtmlElement>().style();
                let _ = style.set_property("--vkb-height", &format!("{}px", viewport_height));
            }
        });

        // Keep a JS function reference for removing listeners in cleanup
        let handler_fn: js_sys::Function =
            handler.as_ref().unchecked_ref::<js_sys::Function>().clone();

        // Listen to resize and scroll on visualViewport
        let add_listener = js_sys::Reflect::get(&vv, &JsValue::from_str("addEventListener"))
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
        if let Some(ref add_fn) = add_listener {
            let _ = add_fn.call2(
                &vv,
                &JsValue::from_str("resize"),
                handler.as_ref().unchecked_ref(),
            );
            let _ = add_fn.call2(
                &vv,
                &JsValue::from_str("scroll"),
                handler.as_ref().unchecked_ref(),
            );
        }

        // .forget() leaks the Closure's Wasm allocation but is required because
        // on_cleanup requires Send+Sync and Closure<dyn Fn> is neither.
        // The listeners themselves are properly removed in cleanup.
        handler.forget();

        // Cleanup: remove listeners when effect re-runs or owner is disposed
        let vv_cleanup = vv.clone();
        on_cleanup(move || {
            if let Some(remove_fn) =
                js_sys::Reflect::get(&vv_cleanup, &JsValue::from_str("removeEventListener"))
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
            {
                let _ = remove_fn.call2(&vv_cleanup, &JsValue::from_str("resize"), &handler_fn);
                let _ = remove_fn.call2(&vv_cleanup, &JsValue::from_str("scroll"), &handler_fn);
            }
        });
    });
}
