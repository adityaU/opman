//! NativeTermView component — DOM-rendered terminal using vt100 crate.
//! Screen state and rendering logic live in `screen.rs`.
//! Includes a hidden textarea proxy so mobile/tablet software keyboards open.

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub use super::screen::TermScreen;
use crate::components::debug_overlay::dbg_log;

/// Native terminal view component — renders the vt100 screen as DOM.
#[component]
pub fn NativeTermView(
    /// The terminal screen state to render.
    screen: SendWrapper<TermScreen>,
    /// Revision signal — bumped on each screen update to trigger re-render.
    revision: ReadSignal<u64>,
    /// Called when user types a key.
    #[prop(into)]
    on_input: Callback<String>,
    /// Called when the container resizes — provides (rows, cols).
    #[prop(into, optional)]
    on_resize: Option<Callback<(u16, u16)>>,
    /// Current search query (empty = no search).
    #[prop(into, optional)]
    search_query: Option<Signal<String>>,
    /// Index of the currently active search match (None = none active).
    #[prop(into, optional)]
    search_active_idx: Option<Signal<Option<usize>>>,
) -> impl IntoView {
    let screen_r = screen.clone();
    let container_ref = NodeRef::<leptos::html::Div>::new();
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();

    // ResizeObserver — compute rows/cols from container pixel size
    {
        let on_resize = on_resize;
        Effect::new(move |_| {
            let Some(el) = container_ref.get() else {
                return;
            };
            let el: &web_sys::HtmlElement = &el;
            let on_resize = on_resize;

            let cb = Closure::<dyn Fn(js_sys::Array)>::new(move |entries: js_sys::Array| {
                let Some(on_resize) = on_resize else {
                    return;
                };

                // Direct VKB check — bypass the reactive signal to avoid
                // the race where ResizeObserver fires before
                // use_virtual_keyboard has set vkb_open to true.
                if is_vkb_open_direct() {
                    dbg_log("[TERM-NATIVE] ResizeObserver suppressed (VKB open, direct check)");
                    return;
                }

                let entry: web_sys::ResizeObserverEntry = entries.get(0).unchecked_into();
                let rect = entry.content_rect();
                let w = rect.width();
                let h = rect.height();
                dbg_log(&format!(
                    "[TERM-NATIVE] ResizeObserver fired: w={:.0}, h={:.0}",
                    w, h
                ));
                if w < 1.0 || h < 1.0 {
                    return;
                }
                // Font metrics: 13px * 0.6 char-width ratio, 1.2 line-height
                let cell_w: f64 = 13.0 * 0.6;
                let cell_h: f64 = 13.0 * 1.2;
                let cols = ((w - 8.0) / cell_w).floor().max(1.0) as u16; // 8px padding
                let rows = ((h - 8.0) / cell_h).floor().max(1.0) as u16;
                on_resize.run((rows, cols));
            });

            let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())
                .expect("ResizeObserver supported");
            observer.observe(el);
            cb.forget();

            on_cleanup(move || {
                observer.disconnect();
            });
        });
    }

    // Keyboard handler — capture raw keys for the PTY (physical keyboards
    // and mobile special keys like arrows, Enter, Backspace).
    let on_keydown = {
        let on_input = on_input.clone();
        move |e: web_sys::KeyboardEvent| {
            let key = e.key();
            let ctrl = e.ctrl_key() || e.meta_key();

            // IME composing — let the `input` event handle it
            if key == "Unidentified" || key == "Process" {
                return;
            }

            // Let browser handle Ctrl+C copy when there's a selection
            if ctrl && key == "c" {
                let sel = web_sys::window()
                    .and_then(|w| w.get_selection().ok().flatten())
                    .map(|s| s.to_string().as_string().unwrap_or_default())
                    .unwrap_or_default();
                if !sel.is_empty() {
                    return;
                }
            }

            let data = key_to_terminal_input(&key, ctrl, e.shift_key(), e.alt_key());
            if !data.is_empty() {
                e.prevent_default();
                on_input.run(data);
            }
        }
    };

    // Mobile software keyboard `input` event — captures typed text from
    // the hidden textarea.
    let on_textarea_input = {
        let on_input = on_input.clone();
        move |_e: web_sys::Event| {
            let Some(ta) = textarea_ref.get() else { return };
            let el: &web_sys::HtmlTextAreaElement = &ta;
            let val = el.value();
            if val.is_empty() {
                return;
            }
            el.set_value("");
            on_input.run(val);
        }
    };

    // `beforeinput` handler — intercept mobile-specific input types that
    // don't fire a useful keydown (e.g. deleteContentBackward on Android).
    let on_before_input = {
        let on_input = on_input.clone();
        move |e: web_sys::InputEvent| {
            match e.input_type().as_str() {
                "deleteContentBackward" => {
                    e.prevent_default();
                    on_input.run("\x7f".to_string()); // Backspace
                }
                "deleteContentForward" => {
                    e.prevent_default();
                    on_input.run("\x1b[3~".to_string()); // Delete
                }
                "insertLineBreak" => {
                    e.prevent_default();
                    on_input.run("\r".to_string()); // Enter
                }
                _ => {}
            }
        }
    };

    // Click handler — focus hidden textarea to open software keyboard
    let on_click = move |_e: web_sys::MouseEvent| {
        dbg_log("[TERM-NATIVE] click -> focusing hidden textarea");
        if let Some(ta) = textarea_ref.get() {
            let el: &web_sys::HtmlElement = &ta;
            let _ = el.focus();
        }
    };

    view! {
        <div
            node_ref=container_ref
            class="native-terminal"
            tabindex="-1"
            on:keydown=on_keydown
            on:click=on_click
            style="width:100%;height:100%;overflow:hidden;outline:none;\
                   font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace;\
                   font-size:13px;line-height:1.2;padding:4px;position:relative;\
                   background:transparent;color:var(--color-text,#e0e0e0);"
        >
            // Hidden textarea — captures mobile software keyboard input.
            <textarea
                node_ref=textarea_ref
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                aria-hidden="true"
                tabindex="0"
                on:input=on_textarea_input
                on:beforeinput=on_before_input
                style="position:absolute;left:0;top:0;width:1px;height:1px;\
                       opacity:0;padding:0;border:none;outline:none;\
                       resize:none;overflow:hidden;z-index:-1;\
                       caret-color:transparent;font-size:16px;"
            />
            // Single container with inner_html — avoids per-line DOM
            // destruction/recreation when the reactive closure re-runs.
            <div
                class="native-term-lines"
                style="white-space:pre;line-height:1.2;"
                inner_html=move || {
                    let _rev = revision.get();
                    let query = search_query.map(|s| s.get()).unwrap_or_default();
                    let active_idx = search_active_idx.and_then(|s| s.get());

                    let highlights: Vec<(usize, usize, usize, bool)> = if query.is_empty() {
                        Vec::new()
                    } else {
                        let matches = screen_r.search_visible(&query);
                        matches
                            .iter()
                            .enumerate()
                            .map(|(i, &(row, cs, ce))| (row, cs, ce, Some(i) == active_idx))
                            .collect()
                    };

                    let lines = screen_r.render_lines(&highlights);
                    let mut buf = String::with_capacity(lines.len() * 80);
                    for html in &lines {
                        buf.push_str("<div class=\"native-term-line\" style=\"height:1.2em;overflow:hidden;\">");
                        buf.push_str(html);
                        buf.push_str("</div>");
                    }
                    buf
                }
            />
        </div>
    }
}

/// Check if the virtual keyboard is currently open by reading the
/// `visualViewport` height directly. This avoids the race condition where
/// `ResizeObserver` fires before the reactive `vkb_open` signal is updated.
fn is_vkb_open_direct() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let inner_h = window
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    if inner_h == 0.0 {
        return false;
    }
    let vv = js_sys::Reflect::get(&window, &JsValue::from_str("visualViewport"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null());
    let Some(vv) = vv else {
        return false;
    };
    let viewport_h = js_sys::Reflect::get(&vv, &JsValue::from_str("height"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(inner_h);
    // Same threshold as use_virtual_keyboard (150px)
    inner_h - viewport_h > 150.0
}

/// Map browser key events to terminal escape sequences.
fn key_to_terminal_input(key: &str, ctrl: bool, _shift: bool, alt: bool) -> String {
    if ctrl {
        if key.len() == 1 {
            let c = key.chars().next().unwrap().to_ascii_lowercase();
            if c.is_ascii_lowercase() {
                return String::from((c as u8 - b'a' + 1) as char);
            }
        }
        return String::new();
    }
    if alt && key.len() == 1 {
        return format!("\x1b{key}");
    }
    match key {
        "Enter" => "\r".to_string(),
        "Backspace" => "\x7f".to_string(),
        "Tab" => "\t".to_string(),
        "Escape" => "\x1b".to_string(),
        "ArrowUp" => "\x1b[A".to_string(),
        "ArrowDown" => "\x1b[B".to_string(),
        "ArrowRight" => "\x1b[C".to_string(),
        "ArrowLeft" => "\x1b[D".to_string(),
        "Home" => "\x1b[H".to_string(),
        "End" => "\x1b[F".to_string(),
        "PageUp" => "\x1b[5~".to_string(),
        "PageDown" => "\x1b[6~".to_string(),
        "Delete" => "\x1b[3~".to_string(),
        "Insert" => "\x1b[2~".to_string(),
        "F1" => "\x1bOP".to_string(),
        "F2" => "\x1bOQ".to_string(),
        "F3" => "\x1bOR".to_string(),
        "F4" => "\x1bOS".to_string(),
        "F5" => "\x1b[15~".to_string(),
        "F6" => "\x1b[17~".to_string(),
        "F7" => "\x1b[18~".to_string(),
        "F8" => "\x1b[19~".to_string(),
        "F9" => "\x1b[20~".to_string(),
        "F10" => "\x1b[21~".to_string(),
        "F11" => "\x1b[23~".to_string(),
        "F12" => "\x1b[24~".to_string(),
        other => {
            if other.len() == 1 || other.chars().count() == 1 {
                other.to_string()
            } else {
                String::new()
            }
        }
    }
}
