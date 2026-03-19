//! Virtual keyboard detection hook — sets CSS/data attributes on <html>.
//! Matches React `useVirtualKeyboard.ts`.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::debug_overlay::dbg_log;

/// Detect mobile virtual keyboard open/close via the Visual Viewport API.
/// Sets `data-vkb-open` attribute and `--vkb-height` CSS variable on `<html>`.
///
/// Returns a `ReadSignal<bool>` that is `true` while the keyboard is open.
/// Components can use this to suppress layout-driven side-effects (e.g.
/// terminal ResizeObserver recalculations).
pub fn use_virtual_keyboard() -> ReadSignal<bool> {
    let (is_open, set_is_open) = signal(false);

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

        let doc_el = window.document().and_then(|d| d.document_element());

        let doc_el_resize = doc_el.clone();
        let vv_resize = vv.clone();
        let set_open = set_is_open;

        // Track previous open state to avoid redundant DOM writes
        // (matches React's openRef pattern).
        let was_open = std::rc::Rc::new(std::cell::Cell::new(false));
        let was_open_cb = was_open.clone();

        let handler = Closure::<dyn Fn()>::new(move || {
            // Read window.innerHeight fresh each time (it stays constant
            // on mobile while visualViewport.height shrinks for keyboard).
            let win = match web_sys::window() {
                Some(w) => w,
                None => return,
            };
            let inner_h = win
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let viewport_height = js_sys::Reflect::get(&vv_resize, &JsValue::from_str("height"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(inner_h);

            let threshold = 150.0;
            let open_now = inner_h - viewport_height > threshold;

            // Only touch the DOM when the state changes
            if open_now != was_open_cb.get() {
                was_open_cb.set(open_now);
                dbg_log(&format!(
                    "[VKB] is_open changed: {} -> {} (innerH={:.0}, viewH={:.0}, diff={:.0})",
                    !open_now,
                    open_now,
                    inner_h,
                    viewport_height,
                    inner_h - viewport_height
                ));

                // Set DOM attribute BEFORE the reactive signal so that
                // synchronous callbacks (e.g. ResizeObserver) that check
                // `data-vkb-open` or `is_vkb_open_direct()` see the new
                // state immediately, avoiding the VKB race condition.
                if let Some(ref el) = doc_el_resize {
                    if open_now {
                        let _ = el.set_attribute("data-vkb-open", "");
                    } else {
                        let _ = el.remove_attribute("data-vkb-open");
                        let style = el.unchecked_ref::<web_sys::HtmlElement>().style();
                        let _ = style.remove_property("--vkb-height");
                    }
                }

                set_open.set(open_now);
            }

            // While open, continuously update available height
            if open_now {
                if let Some(ref el) = doc_el_resize {
                    let style = el.unchecked_ref::<web_sys::HtmlElement>().style();
                    let _ = style
                        .set_property("--vkb-height", &format!("{}px", viewport_height.round()));
                }
            }
        });

        // Keep a JS function reference for cleanup
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
        handler.forget();

        // Cleanup: remove listeners + DOM attributes when owner is disposed.
        // NOTE: handler_fn (js_sys::Function) is Send+Sync. The Rc<Cell>
        // (`was_open`) is not, so we don't capture it in cleanup — the
        // handler Closure was already `.forget()`-ed above.
        let vv_cleanup = vv.clone();
        let doc_el_cleanup = doc_el.clone();
        on_cleanup(move || {
            if let Some(remove_fn) =
                js_sys::Reflect::get(&vv_cleanup, &JsValue::from_str("removeEventListener"))
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
            {
                let _ = remove_fn.call2(&vv_cleanup, &JsValue::from_str("resize"), &handler_fn);
                let _ = remove_fn.call2(&vv_cleanup, &JsValue::from_str("scroll"), &handler_fn);
            }
            if let Some(el) = doc_el_cleanup {
                let _ = el.remove_attribute("data-vkb-open");
                let style = el.unchecked_ref::<web_sys::HtmlElement>().style();
                let _ = style.remove_property("--vkb-height");
            }
        });
    });

    is_open
}
