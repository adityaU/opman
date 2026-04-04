//! A2UI — agent-to-UI block renderer for `ui_render` tool calls.
//!
//! Renders structured UI blocks (cards, tables, key-value pairs, status
//! indicators, progress bars, alerts, markdown, buttons, forms, steps,
//! dividers, code, metrics) inline in the session timeline.
//!
//! Delta updates (render_id + operation) are handled upstream by
//! `render_interleaved()` which pre-merges blocks from all parts sharing
//! a render_id and passes the merged input here. This component is fully
//! stateless — no thread-local storage, no side effects.
//!
//! **Inner-HTML rendering**: all blocks are rendered to HTML strings and
//! injected via `inner_html`. This avoids Leptos fragment accumulation
//! that occurs when `collect_view()` is used inside a reactive closure
//! (the streaming last-group re-creates the component on each SSE event,
//! causing fragments to append rather than replace).
//!
//! Button / form interactivity is preserved through event delegation:
//! a one-time `Effect` attaches click/submit listeners to the container
//! node, dispatching `opman:a2ui-callback` custom events.

mod blocks;
mod blocks_ext;
mod interactive;

mod html_render;
mod html_render_chart;
mod html_render_chart_ext;
mod html_render_coding;
mod html_render_content;
mod html_render_ext;
mod html_render_icons;
mod html_render_interface;
mod html_render_layout;
mod html_render_media;

use leptos::prelude::*;

// ── Top-level component ────────────────────────────────────────────

/// Top-level A2UI renderer. Extracts `blocks` array from tool input,
/// renders them to an HTML string, and uses `inner_html` to inject.
/// A post-mount effect wires event delegation for buttons/forms.
#[component]
pub fn A2uiBlocks(input: serde_json::Value) -> impl IntoView {
    let blocks = extract_blocks(&input);
    if blocks.is_empty() {
        return view! {}.into_any();
    }

    let title = input
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let blocks_html = html_render::blocks_to_html(&blocks);

    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Wire event delegation after the inner_html content is mounted.
    Effect::new(move |_| {
        let Some(el) = container_ref.get() else {
            return;
        };
        wire_a2ui_events(&el);
    });

    view! {
        <div class="a2ui-container">
            {title.map(|t| view! { <div class="a2ui-title">{t}</div> })}
            <div
                class="a2ui-blocks-inner"
                node_ref=container_ref
                inner_html=blocks_html
            />
        </div>
    }
    .into_any()
}

fn extract_blocks(input: &serde_json::Value) -> Vec<serde_json::Value> {
    let src = if let Some(obj) = input.as_object() {
        obj.get("blocks").cloned().unwrap_or_default()
    } else {
        input.clone()
    };
    src.as_array().cloned().unwrap_or_default()
}

// ── Event delegation for buttons / forms ────────────────────────────

/// Attach delegated click (button) and submit (form) handlers to the
/// container element. Uses `data-a2ui-callback` and
/// `data-a2ui-form-callback` attributes set by the HTML renderers.
fn wire_a2ui_events(el: &web_sys::HtmlDivElement) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let el_clone = el.clone();

    // Button clicks — delegated via `data-a2ui-callback`
    let click_cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |ev: web_sys::MouseEvent| {
        let Some(target) = ev.target() else { return };
        let target: web_sys::Element = match target.dyn_into::<web_sys::Element>() {
            Ok(e) => e,
            Err(_) => return,
        };
        // Walk up to find the button with the data attribute
        let mut node = Some(target);
        while let Some(ref n) = node {
            if let Some(cb_id) = n.get_attribute("data-a2ui-callback") {
                if !cb_id.is_empty() {
                    interactive::fire_a2ui_callback(&cb_id, serde_json::Value::Null);
                    // Disable the button and show "Sent"
                    let _ = n.set_attribute("disabled", "true");
                    n.set_inner_html("<span class=\"a2ui-btn-done\">Sent</span>");
                }
                return;
            }
            node = n.parent_element();
        }
    });
    let _ = el.add_event_listener_with_callback("click", click_cb.as_ref().unchecked_ref());
    click_cb.forget(); // leak — lives as long as the DOM node

    // Form submits — delegated via `data-a2ui-form-callback`
    let el_for_submit = el_clone;
    let submit_cb =
        Closure::<dyn Fn(web_sys::SubmitEvent)>::new(move |ev: web_sys::SubmitEvent| {
            ev.prevent_default();
            let Some(target) = ev.target() else { return };
            let form: web_sys::HtmlFormElement = match target.dyn_into() {
                Ok(f) => f,
                Err(_) => return,
            };
            let cb_id = form
                .get_attribute("data-a2ui-form-callback")
                .unwrap_or_default();
            if cb_id.is_empty() {
                return;
            }

            // Collect form field values
            let mut values = serde_json::Map::new();
            let elements = form.elements();
            for i in 0..elements.length() {
                let Some(item) = elements.item(i) else {
                    continue;
                };
                if let Ok(input) = item.clone().dyn_into::<web_sys::HtmlInputElement>() {
                    let name = input.name();
                    if !name.is_empty() {
                        values.insert(name, serde_json::Value::String(input.value()));
                    }
                } else if let Ok(ta) = item.dyn_into::<web_sys::HtmlTextAreaElement>() {
                    let name = ta.name();
                    if !name.is_empty() {
                        values.insert(name, serde_json::Value::String(ta.value()));
                    }
                }
            }

            interactive::fire_a2ui_callback(&cb_id, serde_json::Value::Object(values));

            // Disable all inputs + change submit button text
            let inputs = form.elements();
            for i in 0..inputs.length() {
                let Some(item) = inputs.item(i) else {
                    continue;
                };
                if let Ok(inp) = item.clone().dyn_into::<web_sys::HtmlInputElement>() {
                    inp.set_disabled(true);
                } else if let Ok(ta) = item.clone().dyn_into::<web_sys::HtmlTextAreaElement>() {
                    ta.set_disabled(true);
                } else if let Ok(btn) = item.dyn_into::<web_sys::HtmlButtonElement>() {
                    btn.set_disabled(true);
                    btn.set_inner_html("<span class=\"a2ui-btn-done\">Submitted</span>");
                }
            }
        });
    let _ = el_for_submit
        .add_event_listener_with_callback("submit", submit_cb.as_ref().unchecked_ref());
    submit_cb.forget(); // leak — lives as long as the DOM node

    // Mermaid diagrams — run mermaid.js on any <pre class="mermaid"> elements.
    run_mermaid(el);
}

/// Initialize mermaid.js and render any `<pre class="mermaid">` elements
/// inside the given container. Mermaid is loaded globally via CDN in
/// index.html; this just calls `mermaid.run({ nodes: [...] })`.
fn run_mermaid(el: &web_sys::HtmlDivElement) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let nodes = el.query_selector_all("pre.mermaid");
    let Ok(nodes) = nodes else { return };
    if nodes.length() == 0 {
        return;
    }

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let mermaid = js_sys::Reflect::get(&window, &JsValue::from_str("mermaid"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null());
    let Some(m) = mermaid else {
        log::warn!("A2UI mermaid block: mermaid.js not loaded");
        return;
    };

    // mermaid.initialize({ startOnLoad: false, theme: 'dark', securityLevel: 'strict' })
    if let Ok(init_fn) = js_sys::Reflect::get(&m, &JsValue::from_str("initialize")) {
        if init_fn.is_function() {
            let func: js_sys::Function = init_fn.unchecked_into();
            let config = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&config, &"startOnLoad".into(), &JsValue::FALSE);
            let _ = js_sys::Reflect::set(&config, &"theme".into(), &"dark".into());
            let _ = js_sys::Reflect::set(&config, &"securityLevel".into(), &"strict".into());
            let _ = func.call1(&m, &config);
        }
    }

    // mermaid.run({ nodes: [pre1, pre2, ...] })
    let run_fn = js_sys::Reflect::get(&m, &JsValue::from_str("run"))
        .ok()
        .filter(|v| v.is_function());
    let Some(run_fn) = run_fn else { return };
    let func: js_sys::Function = run_fn.unchecked_into();

    let node_array = js_sys::Array::new();
    for i in 0..nodes.length() {
        if let Some(n) = nodes.item(i) {
            node_array.push(&n);
        }
    }
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"nodes".into(), &node_array);
    let _ = func.call1(&m, &opts);
}
