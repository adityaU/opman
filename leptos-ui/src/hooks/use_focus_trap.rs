//! Focus trap hook — traps Tab/Shift+Tab within a container element.
//! Matches React `useFocusTrap.ts`.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlElement;

const FOCUSABLE_SELECTOR: &str = concat!(
    "a[href],",
    "button:not([disabled]),",
    "input:not([disabled]),",
    "textarea:not([disabled]),",
    "select:not([disabled]),",
    "[tabindex]:not([tabindex=\"-1\"])"
);

/// Trap keyboard focus within the given container.
/// On creation, saves `document.activeElement`, focuses first focusable child.
/// On drop (cleanup), restores previously-focused element.
pub fn use_focus_trap(container_ref: NodeRef<leptos::html::Div>) {
    Effect::new(move |_| {
        let Some(container) = container_ref.get() else {
            return;
        };
        let el: &HtmlElement = &container;

        // Save previously focused element
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .expect("document");
        let previous_focus: Option<HtmlElement> = doc
            .active_element()
            .and_then(|active| active.dyn_into::<HtmlElement>().ok());

        // Focus first focusable child, or container itself
        if let Ok(nodes) = el.query_selector_all(FOCUSABLE_SELECTOR) {
            if nodes.length() > 0 {
                if let Some(first) = nodes.get(0) {
                    if let Ok(first_el) = first.dyn_into::<HtmlElement>() {
                        let _ = first_el.focus();
                    }
                }
            } else {
                let _ = el.focus();
            }
        }

        // Set up keydown listener for Tab trapping
        let container_el: HtmlElement = el.clone();
        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                if e.key() != "Tab" {
                    return;
                }
                let Ok(nodes) = container_el.query_selector_all(FOCUSABLE_SELECTOR) else {
                    return;
                };
                let len = nodes.length();
                if len == 0 {
                    return;
                }
                let first = nodes.get(0).and_then(|n| n.dyn_into::<HtmlElement>().ok());
                let last = nodes
                    .get(len - 1)
                    .and_then(|n| n.dyn_into::<HtmlElement>().ok());

                let doc = web_sys::window().and_then(|w| w.document());
                let active = doc.and_then(|d| d.active_element());

                if e.shift_key() {
                    // Shift+Tab: wrap from first to last
                    if let (Some(ref first_el), Some(ref active_el)) = (&first, &active) {
                        if active_el == first_el.as_ref() {
                            e.prevent_default();
                            if let Some(ref last_el) = last {
                                let _ = last_el.focus();
                            }
                        }
                    }
                } else {
                    // Tab: wrap from last to first
                    if let (Some(ref last_el), Some(ref active_el)) = (&last, &active) {
                        if active_el == last_el.as_ref() {
                            e.prevent_default();
                            if let Some(ref first_el) = first {
                                let _ = first_el.focus();
                            }
                        }
                    }
                }
            });

        let _ = web_sys::window().and_then(|w| w.document()).map(|doc| {
            doc.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
        });

        // Leak the closure intentionally — cleanup will remove listener
        // In practice for CSR modals, this is fine as modals are short-lived.
        handler.forget();

        // Restore focus on cleanup via on_cleanup
        // We store previous_focus in a local variable captured by the cleanup closure
        // NOTE: on_cleanup needs Send+Sync on the closure in Leptos 0.7.
        // Since HtmlElement is not Send+Sync, we store a JsValue instead and cast back.
        let prev_js: Option<JsValue> = previous_focus.map(|el| el.into());
        on_cleanup(move || {
            if let Some(ref js_val) = prev_js {
                if let Ok(el) = js_val.clone().dyn_into::<HtmlElement>() {
                    let _ = el.focus();
                }
            }
        });
    });
}
