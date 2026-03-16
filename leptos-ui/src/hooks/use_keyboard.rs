//! Global keyboard shortcut system.
//! Matches React `useKeyboard.ts` behavior.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// A single keyboard binding.
#[derive(Clone)]
pub struct KeyBinding {
    pub key: String,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub alt: bool,
    pub handler: Callback<()>,
    pub description: Option<String>,
}

impl KeyBinding {
    pub fn new(key: &str, handler: Callback<()>) -> Self {
        Self {
            key: key.to_string(),
            ctrl: false,
            meta: false,
            shift: false,
            alt: false,
            handler,
            description: None,
        }
    }

    pub fn meta(mut self) -> Self {
        self.meta = true;
        self
    }

    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub fn desc(mut self, d: &str) -> Self {
        self.description = Some(d.to_string());
        self
    }
}

/// Register global keyboard shortcuts.
/// Bindings are matched against keydown events on the document.
/// The listener is cleaned up when the owning reactive scope is disposed.
pub fn use_keyboard(bindings: Vec<KeyBinding>) {
    let bindings = StoredValue::new(bindings);

    let document = web_sys::window()
        .and_then(|w| w.document())
        .expect("no document");

    let cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
        let bindings = bindings.get_value();
        let target = e.target();
        let is_input = target
            .as_ref()
            .and_then(|t| t.dyn_ref::<web_sys::HtmlElement>())
            .map(|el| {
                let tag = el.tag_name().to_uppercase();
                tag == "INPUT" || tag == "TEXTAREA" || el.is_content_editable()
            })
            .unwrap_or(false);

        for binding in &bindings {
            let meta_match = if binding.meta {
                e.meta_key() || e.ctrl_key()
            } else {
                true
            };
            let ctrl_match = if binding.ctrl { e.ctrl_key() } else { true };
            let shift_match = if binding.shift {
                e.shift_key()
            } else {
                !e.shift_key()
            };
            let alt_match = if binding.alt {
                e.alt_key()
            } else {
                !e.alt_key()
            };
            let key_match = e.key().to_lowercase() == binding.key.to_lowercase();

            // For non-modifier combos in inputs, skip (except Escape)
            if is_input && !binding.meta && !binding.ctrl && binding.key.to_lowercase() != "escape"
            {
                continue;
            }

            if key_match && meta_match && ctrl_match && shift_match && alt_match {
                if binding.meta && !(e.meta_key() || e.ctrl_key()) {
                    continue;
                }
                if binding.ctrl && !e.ctrl_key() {
                    continue;
                }

                e.prevent_default();
                e.stop_propagation();
                binding.handler.run(());
                return;
            }
        }
    });

    let _ = document.add_event_listener_with_callback_and_bool(
        "keydown",
        cb.as_ref().unchecked_ref(),
        true, // capture
    );

    // Store the JS function reference for cleanup
    let js_fn: js_sys::Function = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
    // Prevent the closure from being deallocated (it's still referenced by the event listener)
    cb.forget();

    // Remove the listener when the owning scope is disposed
    let doc_clone = document.clone();
    on_cleanup(move || {
        let _ = doc_clone.remove_event_listener_with_callback_and_bool("keydown", &js_fn, true);
    });
}

/// Simple Escape key handler.
pub fn use_escape(handler: Callback<()>) {
    use_keyboard(vec![KeyBinding::new("Escape", handler)]);
}
