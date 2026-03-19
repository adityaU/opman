//! DOM-based renderer for the native code editor.
//! Renders line numbers, syntax-highlighted code, and cursor via Leptos views.
//! Uses viewport windowing — only visible lines (+ buffer) are rendered.
//! Includes a hidden textarea proxy so mobile/tablet software keyboards open.

use std::cell::Cell;
use std::rc::Rc;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::buffer_state::EditorBuffer;
use super::helpers::{
    find_line_element, handle_movement, is_movement_key, notify_cursor, render_line,
};
use super::highlighter::SyntaxHighlighter;
use super::input::{map_key, InputAction};
use crate::components::debug_overlay::dbg_log;
use floem_editor_core::command::EditCommand;

/// Lines of overscan above/below the viewport.
const OVERSCAN: usize = 20;
/// Line height in px (13px font * 1.5 line-height).
const LINE_HEIGHT: f64 = 19.5;
/// Debounce delay for on_change (ms).
const CHANGE_DEBOUNCE_MS: i32 = 80;

/// Native code editor component — replaces CodeMirror.
#[component]
pub fn NativeEditor(
    /// Initial file content.
    #[prop(into)]
    content: String,
    /// File extension for syntax highlighting (e.g. "rs", "ts").
    #[prop(into)]
    extension: String,
    /// Called whenever content changes.
    #[prop(into)]
    on_change: Callback<String>,
    /// Called whenever cursor position changes (line, col) — 1-indexed.
    #[prop(into)]
    on_cursor: Callback<(u32, u32)>,
    /// Optional line to jump to (1-indexed).
    #[prop(into, optional)]
    jump_to_line: Option<Signal<Option<u32>>>,
) -> impl IntoView {
    dbg_log(&format!(
        "[NATIVE-EDITOR] NativeEditor constructor called for ext={}",
        extension
    ));
    let buffer = SendWrapper::new(EditorBuffer::new(&content));
    let highlighter = SendWrapper::new(SyntaxHighlighter::new(&extension));
    let (revision, set_revision) = signal(0u64);
    let (scroll_top, set_scroll_top) = signal(0.0f64);
    let (view_height, set_view_height) = signal(600.0f64);
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Debounced on_change — avoids full-string extraction every keystroke.
    let debounce_timer: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    let schedule_change = {
        let buf = buffer.clone();
        let timer = debounce_timer.clone();
        move || {
            // Cancel previous timer
            if let Some(id) = timer.get() {
                let _ = web_sys::window().unwrap().clear_timeout_with_handle(id);
            }
            let buf = buf.clone();
            let on_change = on_change;
            let cb = Closure::once(move || {
                on_change.run(buf.content());
            });
            let id = web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    CHANGE_DEBOUNCE_MS,
                )
                .unwrap_or(0);
            cb.forget();
            timer.set(Some(id));
        }
    };

    // Helper: perform an edit, invalidate highlighting from cursor line, bump revision.
    let do_edit_cycle = {
        let buf = buffer.clone();
        let hl = highlighter.clone();
        let sched = schedule_change.clone();
        move || {
            let line = buf.cursor_pos().0;
            hl.invalidate_from(Some(line));
            set_revision.update(|r| *r += 1);
            sched();
            notify_cursor(&buf, &on_cursor);
        }
    };

    // Handle jump-to-line
    if let Some(jl_signal) = jump_to_line {
        let buf = buffer.clone();
        Effect::new(move |_| {
            if let Some(line) = jl_signal.get() {
                if line > 0 {
                    buf.jump_to_line((line - 1) as usize);
                    set_revision.update(|r| *r += 1);
                }
            }
        });
    }

    // Keyboard handler
    let buf_key = buffer.clone();
    let hl_key = highlighter.clone();
    let sched_key = schedule_change.clone();
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        let ctrl = e.ctrl_key() || e.meta_key();
        let shift = e.shift_key();

        if ctrl && (key == "s" || key == "a") {
            return;
        }
        if key == "Unidentified" || key == "Process" {
            return;
        }
        if is_movement_key(&key) {
            handle_movement(&buf_key, &key, ctrl);
            set_revision.update(|r| *r += 1);
            notify_cursor(&buf_key, &on_cursor);
            e.prevent_default();
            return;
        }

        match map_key(&key, ctrl, shift) {
            InputAction::Insert(s) => {
                e.prevent_default();
                let line = buf_key.cursor_pos().0;
                buf_key.insert(&s);
                hl_key.invalidate_from(Some(line));
                set_revision.update(|r| *r += 1);
                sched_key();
                notify_cursor(&buf_key, &on_cursor);
            }
            InputAction::Command(cmd) => {
                e.prevent_default();
                let line = buf_key.cursor_pos().0;
                buf_key.do_edit(&cmd);
                hl_key.invalidate_from(Some(line));
                set_revision.update(|r| *r += 1);
                sched_key();
                notify_cursor(&buf_key, &on_cursor);
            }
            InputAction::None => {}
        }
    };

    // Mobile textarea input handler
    let buf_input = buffer.clone();
    let hl_input = highlighter.clone();
    let sched_input = schedule_change.clone();
    let on_textarea_input = move |_e: web_sys::Event| {
        let Some(ta) = textarea_ref.get() else { return };
        let el: &web_sys::HtmlTextAreaElement = &ta;
        let val = el.value();
        if val.is_empty() {
            return;
        }
        el.set_value("");
        let line = buf_input.cursor_pos().0;
        buf_input.insert(&val);
        hl_input.invalidate_from(Some(line));
        set_revision.update(|r| *r += 1);
        sched_input();
        notify_cursor(&buf_input, &on_cursor);
    };

    // beforeinput handler for mobile delete/newline
    let buf_bi = buffer.clone();
    let do_edit_bi = do_edit_cycle.clone();
    let on_before_input = move |e: web_sys::InputEvent| match e.input_type().as_str() {
        "deleteContentBackward" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::DeleteBackward);
            do_edit_bi();
        }
        "deleteContentForward" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::DeleteForward);
            do_edit_bi();
        }
        "insertLineBreak" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::InsertNewLine);
            do_edit_bi();
        }
        _ => {}
    };

    // Click handler — position cursor + focus textarea
    let buf_click = buffer.clone();
    let on_click = move |e: web_sys::MouseEvent| {
        dbg_log("[NATIVE-EDITOR] click -> focusing hidden textarea");
        if let Some(ta) = textarea_ref.get() {
            let el: &web_sys::HtmlElement = &ta;
            let _ = el.focus();
        }
        let Some(target) = e.target() else { return };
        let Ok(el) = target.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        let Some(line_el) = find_line_element(&el) else {
            return;
        };
        let line_idx = line_el
            .dataset()
            .get("line")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let rect = line_el.get_bounding_client_rect();
        let x_offset = (e.client_x() as f64 - rect.left()).max(0.0);
        let col = (x_offset / 7.8) as usize;
        buf_click.set_cursor_pos(line_idx, col);
        set_revision.update(|r| *r += 1);
        notify_cursor(&buf_click, &on_cursor);
    };

    // Scroll handler — update scroll_top for viewport windowing
    let on_scroll = move |_e: web_sys::Event| {
        let Some(el) = container_ref.get() else {
            return;
        };
        let html: &web_sys::HtmlElement = &el;
        set_scroll_top.set(html.scroll_top() as f64);
        set_view_height.set(html.client_height() as f64);
    };

    // Render view with viewport windowing
    let buf_r = buffer.clone();
    let hl_r = highlighter.clone();

    view! {
        <div
            node_ref=container_ref
            class="native-editor"
            tabindex="-1"
            on:keydown=on_keydown
            on:click=on_click
            on:scroll=on_scroll
            style="width:100%;height:100%;overflow:auto;outline:none;\
                   font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace;\
                   font-size:13px;line-height:1.5;position:relative;\
                   color:var(--color-text,#e0e0e0);"
        >
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
            {move || {
                let _rev = revision.get();
                let st = scroll_top.get();
                let vh = view_height.get();
                let num_lines = buf_r.num_lines();
                let (cursor_line, cursor_col) = buf_r.cursor_pos();

                // Viewport window
                let first = ((st / LINE_HEIGHT) as usize).saturating_sub(OVERSCAN);
                let visible_count = (vh / LINE_HEIGHT) as usize + 1;
                let last = (first + visible_count + OVERSCAN * 2).min(num_lines);

                // Spacer above for lines not rendered
                let top_px = first as f64 * LINE_HEIGHT;
                let total_px = num_lines as f64 * LINE_HEIGHT;
                let bottom_px = (total_px - last as f64 * LINE_HEIGHT).max(0.0);

                view! {
                    <div style=format!("height:{top_px}px;") />
                    {(first..last).map(|idx| {
                        let text = buf_r.line_content(idx);
                        let spans = hl_r.highlight_line(idx, &text);
                        let is_cur = idx == cursor_line;
                        render_line(idx, spans, is_cur, cursor_col)
                    }).collect::<Vec<_>>()}
                    <div style=format!("height:{bottom_px}px;") />
                }
            }}
        </div>
    }
}
