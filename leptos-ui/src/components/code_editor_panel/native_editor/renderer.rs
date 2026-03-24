//! DOM-based renderer for the native code editor.
//! Uses viewport windowing — only visible lines (+ buffer) are rendered.
//! Cursor is updated imperatively to avoid re-rendering the entire DOM on every
//! click / arrow-key, which would destroy browser text selection and cause
//! scroll jumps.

use std::cell::Cell;
use std::rc::Rc;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::buffer_state::EditorBuffer;
use super::helpers::{
    find_line_element, handle_movement, is_movement_key, notify_cursor, render_line,
    update_cursor_in_container,
};
use super::highlighter::SyntaxHighlighter;
use super::input::{map_key, InputAction};
use crate::components::debug_overlay::dbg_log;
use floem_editor_core::command::EditCommand;

const OVERSCAN: usize = 20;
const LINE_HEIGHT: f64 = 19.5;
const CHANGE_DEBOUNCE_MS: i32 = 80;

/// Native code editor component — replaces CodeMirror.
#[component]
pub fn NativeEditor(
    #[prop(into)] content: String,
    #[prop(into)] extension: String,
    #[prop(into)] on_change: Callback<String>,
    #[prop(into)] on_cursor: Callback<(u32, u32)>,
    #[prop(into, optional)] jump_to_line: Option<Signal<Option<u32>>>,
) -> impl IntoView {
    dbg_log(&format!("[NATIVE-EDITOR] constructor ext={}", extension));
    let buffer = SendWrapper::new(EditorBuffer::new(&content));
    let highlighter = SendWrapper::new(SyntaxHighlighter::new(&extension));
    let (content_rev, set_content_rev) = signal(0u64);
    let scroll_top: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
    let view_height: Rc<Cell<f64>> = Rc::new(Cell::new(600.0));
    let (viewport_rev, set_viewport_rev) = signal(0u64);
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let container_ref = NodeRef::<leptos::html::Div>::new();

    let imperative_cursor = {
        let buf = buffer.clone();
        let cr = container_ref;
        Rc::new(move || {
            let Some(c) = cr.get() else { return };
            let (l, c2) = buf.cursor_pos();
            update_cursor_in_container(&c, l, c2);
        })
    };

    let debounce_timer: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    let schedule_change = {
        let buf = buffer.clone();
        let timer = debounce_timer.clone();
        move || {
            if let Some(id) = timer.get() {
                let _ = web_sys::window().unwrap().clear_timeout_with_handle(id);
            }
            let buf = buf.clone();
            let cb = Closure::once(move || on_change.run(buf.content()));
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

    // Content edit: invalidate highlighting, bump content_rev, schedule change.
    let content_edit = {
        let (buf, hl, sched) = (buffer.clone(), highlighter.clone(), schedule_change.clone());
        move || {
            hl.invalidate_from(Some(buf.cursor_pos().0));
            set_content_rev.update(|r| *r += 1);
            sched();
            notify_cursor(&buf, &on_cursor);
        }
    };

    // Cursor-only move: imperative DOM update, no re-render.
    let cursor_move = {
        let (up, buf) = (imperative_cursor.clone(), buffer.clone());
        move || {
            up();
            notify_cursor(&buf, &on_cursor);
        }
    };

    if let Some(jl) = jump_to_line {
        let (buf, up) = (buffer.clone(), imperative_cursor.clone());
        Effect::new(move |_| {
            if let Some(line) = jl.get() {
                if line > 0 {
                    buf.jump_to_line((line - 1) as usize);
                    set_content_rev.update(|r| *r += 1);
                    up();
                }
            }
        });
    }

    let (buf_k, hl_k, sched_k, cm_k) = (
        buffer.clone(),
        highlighter.clone(),
        schedule_change.clone(),
        cursor_move.clone(),
    );
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let (key, ctrl, shift) = (e.key(), e.ctrl_key() || e.meta_key(), e.shift_key());
        if ctrl && (key == "s" || key == "a") {
            return;
        }
        if key == "Unidentified" || key == "Process" {
            return;
        }
        if is_movement_key(&key) {
            handle_movement(&buf_k, &key, ctrl);
            cm_k();
            e.prevent_default();
            return;
        }
        match map_key(&key, ctrl, shift) {
            InputAction::Insert(s) => {
                e.prevent_default();
                let l = buf_k.cursor_pos().0;
                buf_k.insert(&s);
                hl_k.invalidate_from(Some(l));
                set_content_rev.update(|r| *r += 1);
                sched_k();
                notify_cursor(&buf_k, &on_cursor);
            }
            InputAction::Command(cmd) => {
                e.prevent_default();
                let l = buf_k.cursor_pos().0;
                buf_k.do_edit(&cmd);
                hl_k.invalidate_from(Some(l));
                set_content_rev.update(|r| *r += 1);
                sched_k();
                notify_cursor(&buf_k, &on_cursor);
            }
            InputAction::None => {}
        }
    };

    let (buf_c, cm_c) = (buffer.clone(), cursor_move.clone());
    let on_mousedown = move |e: web_sys::MouseEvent| {
        if let Some(ta) = textarea_ref.get() {
            let el: &web_sys::HtmlElement = &ta;
            let _ = el.focus();
        }
        let Some(t) = e.target() else { return };
        let Ok(el) = t.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        let Some(le) = find_line_element(&el) else {
            return;
        };
        let li = le
            .dataset()
            .get("line")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let rect = le.get_bounding_client_rect();
        let col = ((e.client_x() as f64 - rect.left()).max(0.0) / 7.8) as usize;
        buf_c.set_cursor_pos(li, col);
        cm_c();
    };

    let (buf_i, hl_i, sched_i) = (buffer.clone(), highlighter.clone(), schedule_change.clone());
    let on_textarea_input = move |_e: web_sys::Event| {
        let Some(ta) = textarea_ref.get() else { return };
        let el: &web_sys::HtmlTextAreaElement = &ta;
        let val = el.value();
        if val.is_empty() {
            return;
        }
        el.set_value("");
        let l = buf_i.cursor_pos().0;
        buf_i.insert(&val);
        hl_i.invalidate_from(Some(l));
        set_content_rev.update(|r| *r += 1);
        sched_i();
        notify_cursor(&buf_i, &on_cursor);
    };

    let (buf_bi, ce_bi) = (buffer.clone(), content_edit.clone());
    let on_before_input = move |e: web_sys::InputEvent| match e.input_type().as_str() {
        "deleteContentBackward" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::DeleteBackward);
            ce_bi();
        }
        "deleteContentForward" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::DeleteForward);
            ce_bi();
        }
        "insertLineBreak" => {
            e.prevent_default();
            buf_bi.do_edit(&EditCommand::InsertNewLine);
            ce_bi();
        }
        _ => {}
    };

    let (st_c, vh_c) = (scroll_top.clone(), view_height.clone());
    let (pf, pl) = (Rc::new(Cell::new(0usize)), Rc::new(Cell::new(0usize)));
    let on_scroll = move |_e: web_sys::Event| {
        let Some(el) = container_ref.get() else {
            return;
        };
        let h: &web_sys::HtmlElement = &el;
        let (s, v) = (h.scroll_top() as f64, h.client_height() as f64);
        st_c.set(s);
        vh_c.set(v);
        let f = ((s / LINE_HEIGHT) as usize).saturating_sub(OVERSCAN);
        let l = f + (v / LINE_HEIGHT) as usize + 1 + OVERSCAN * 2;
        if f != pf.get() || l != pl.get() {
            pf.set(f);
            pl.set(l);
            set_viewport_rev.update(|r| *r += 1);
        }
    };

    let (buf_r, hl_r) = (buffer.clone(), highlighter.clone());
    let st_r = SendWrapper::new(scroll_top);
    let vh_r = SendWrapper::new(view_height);

    view! {
        <div node_ref=container_ref class="native-editor" tabindex="-1"
            on:keydown=on_keydown on:mousedown=on_mousedown on:scroll=on_scroll
            style="width:100%;height:100%;overflow:auto;outline:none;\
                   font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace;\
                   font-size:13px;line-height:1.5;position:relative;\
                   color:var(--color-text,#e0e0e0);user-select:text;">
            <textarea node_ref=textarea_ref autocomplete="off" autocapitalize="off"
                spellcheck="false" aria-hidden="true" tabindex="0"
                on:input=on_textarea_input on:beforeinput=on_before_input
                style="position:absolute;left:0;top:0;width:1px;height:1px;\
                       opacity:0;padding:0;border:none;outline:none;\
                       resize:none;overflow:hidden;z-index:-1;\
                       caret-color:transparent;font-size:16px;" />
            {move || {
                let _c = content_rev.get();
                let _v = viewport_rev.get();
                let (st, vh) = (st_r.get(), vh_r.get());
                let n = buf_r.num_lines();
                let (cl, cc) = buf_r.cursor_pos();
                let first = ((st / LINE_HEIGHT) as usize).saturating_sub(OVERSCAN);
                let vis = (vh / LINE_HEIGHT) as usize + 1;
                let last = (first + vis + OVERSCAN * 2).min(n);
                let top = first as f64 * LINE_HEIGHT;
                let bot = (n as f64 * LINE_HEIGHT - last as f64 * LINE_HEIGHT).max(0.0);
                view! {
                    <div style=format!("height:{top}px;") />
                    {(first..last).map(|i| {
                        let t = buf_r.line_content(i);
                        let sp = hl_r.highlight_line(i, &t);
                        render_line(i, sp, i == cl, cc)
                    }).collect::<Vec<_>>()}
                    <div style=format!("height:{bot}px;") />
                }
            }}
        </div>
    }
}
