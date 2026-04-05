//! Mermaid diagram post-mount logic: calls `mermaid.run()` to render
//! `<pre class="mermaid">` elements into SVGs, and wires delegated
//! click handlers for the zoom-in / zoom-out / reset toolbar buttons.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Wire mermaid zoom controls and run `mermaid.js` on the container.
pub fn setup(el: &web_sys::HtmlDivElement) {
    wire_zoom(el);
    run(el);
}

/// Delegated click handler for mermaid zoom-in / zoom-out / reset buttons.
/// Reads/writes a `data-zoom` attribute on the `.a2ui-mermaid-viewport` and
/// applies `transform: scale(...)` — pure CSS, no animation libraries.
fn wire_zoom(el: &web_sys::HtmlDivElement) {
    let el_clone = el.clone();
    let cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |ev: web_sys::MouseEvent| {
        let Some(target) = ev.target() else { return };
        let Ok(target) = target.dyn_into::<web_sys::Element>() else {
            return;
        };

        // Walk up to find button with data-a2ui-mermaid-zoom
        let mut node = Some(target);
        let mut action = None;
        while let Some(ref n) = node {
            if let Some(a) = n.get_attribute("data-a2ui-mermaid-zoom") {
                action = Some(a);
                break;
            }
            node = n.parent_element();
        }
        let Some(action) = action else { return };

        // Find the closest .a2ui-mermaid ancestor, then its .a2ui-mermaid-viewport
        let btn = node.unwrap();
        let Some(mermaid_el) = btn.closest(".a2ui-mermaid").ok().flatten() else {
            return;
        };
        let Some(viewport) = mermaid_el
            .query_selector(".a2ui-mermaid-viewport")
            .ok()
            .flatten()
        else {
            return;
        };
        let Ok(vp) = viewport.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };

        let cur: f64 = vp
            .get_attribute("data-zoom")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);

        let next = match action.as_str() {
            "in" => (cur + 0.25).min(3.0),
            "out" => (cur - 0.25).max(0.25),
            _ => 1.0, // reset
        };

        let _ = vp.set_attribute("data-zoom", &next.to_string());
        let _ = vp
            .style()
            .set_property("transform", &format!("scale({})", next));
    });

    let _ = el_clone.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
    cb.forget();
}

/// Initialize mermaid.js and render any `<pre class="mermaid">` elements
/// inside the given container. Mermaid is loaded globally via CDN in
/// index.html; this just calls `mermaid.run({ nodes: [...] })`.
fn run(el: &web_sys::HtmlDivElement) {
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
