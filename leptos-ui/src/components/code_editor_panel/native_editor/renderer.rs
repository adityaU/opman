//! DOM-based renderer for the native code editor.
//! Renders line numbers, syntax-highlighted code, and cursor via Leptos views.

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;

use super::buffer_state::EditorBuffer;
use super::highlighter::SyntaxHighlighter;
use super::input::{map_key, InputAction};

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
    let buffer = SendWrapper::new(EditorBuffer::new(&content));
    let highlighter = SendWrapper::new(SyntaxHighlighter::new(&extension));
    let (revision, set_revision) = signal(0u64);

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
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        let ctrl = e.ctrl_key() || e.meta_key();
        let shift = e.shift_key();

        if ctrl && (key == "s" || key == "a") {
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
                buf_key.insert(&s);
                hl_key.invalidate();
                set_revision.update(|r| *r += 1);
                on_change.run(buf_key.content());
                notify_cursor(&buf_key, &on_cursor);
            }
            InputAction::Command(cmd) => {
                e.prevent_default();
                buf_key.do_edit(&cmd);
                hl_key.invalidate();
                set_revision.update(|r| *r += 1);
                on_change.run(buf_key.content());
                notify_cursor(&buf_key, &on_cursor);
            }
            InputAction::None => {}
        }
    };

    // Click handler — position cursor
    let buf_click = buffer.clone();
    let on_click = move |e: web_sys::MouseEvent| {
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
        let char_width = 7.8; // approximate monospace char width at 13px
        let col = (x_offset / char_width) as usize;

        buf_click.set_cursor_pos(line_idx, col);
        set_revision.update(|r| *r += 1);
        notify_cursor(&buf_click, &on_cursor);
    };

    // Render view — all non-Send state accessed inside SendWrapper
    let buf_r = buffer.clone();
    let hl_r = highlighter.clone();

    view! {
        <div
            class="native-editor"
            tabindex="0"
            on:keydown=on_keydown
            on:click=on_click
            style="width:100%;height:100%;overflow:auto;outline:none;\
                   font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace;\
                   font-size:13px;line-height:1.5;position:relative;\
                   color:var(--color-text,#e0e0e0);"
        >
            {move || {
                let _rev = revision.get();
                let num_lines = buf_r.num_lines();
                let (cursor_line, cursor_col) = buf_r.cursor_pos();

                (0..num_lines).map(|idx| {
                    let text = buf_r.line_content(idx);
                    let spans = hl_r.highlight_line(idx, &text);
                    let is_cur = idx == cursor_line;
                    render_line(idx, spans, is_cur, cursor_col)
                }).collect::<Vec<_>>()
            }}
        </div>
    }
}

/// Render a single editor line (line number + highlighted spans + cursor).
fn render_line(
    line_idx: usize,
    spans: Vec<super::highlighter::StyledSpan>,
    is_cursor_line: bool,
    cursor_col: usize,
) -> impl IntoView {
    let line_num = line_idx + 1;
    let bg = if is_cursor_line {
        "display:flex;background:rgba(255,255,255,0.04);"
    } else {
        "display:flex;"
    };

    view! {
        <div
            class="native-editor-line"
            data-line=line_idx.to_string()
            style=bg
        >
            <span
                class="native-editor-gutter"
                style="display:inline-block;min-width:3.5em;\
                       padding-right:1em;text-align:right;\
                       color:var(--color-text-muted,#666);\
                       user-select:none;flex-shrink:0;"
            >
                {line_num.to_string()}
            </span>
            <span
                class="native-editor-content"
                style="white-space:pre;flex:1;min-width:0;position:relative;"
            >
                {spans.into_iter().map(|sp| {
                    let cls = if sp.token_class.is_empty() {
                        String::new()
                    } else {
                        format!("token {}", sp.token_class)
                    };
                    let style = if sp.bold && sp.italic {
                        "font-weight:bold;font-style:italic;"
                    } else if sp.bold {
                        "font-weight:bold;"
                    } else if sp.italic {
                        "font-style:italic;"
                    } else {
                        ""
                    };
                    view! { <span class=cls style=style>{sp.text}</span> }
                }).collect::<Vec<_>>()}
                {is_cursor_line.then(|| view! {
                    <span
                        class="native-editor-cursor"
                        style=format!(
                            "position:absolute;left:{}ch;top:0;\
                             width:2px;height:1.5em;\
                             background:var(--color-text,#e0e0e0);\
                             animation:blink 1s step-end infinite;",
                            cursor_col
                        )
                    />
                })}
            </span>
        </div>
    }
}

fn is_movement_key(key: &str) -> bool {
    matches!(
        key,
        "ArrowLeft"
            | "ArrowRight"
            | "ArrowUp"
            | "ArrowDown"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
    )
}

/// Walk up DOM to find the line container element.
fn find_line_element(el: &web_sys::HtmlElement) -> Option<web_sys::HtmlElement> {
    let mut current: web_sys::HtmlElement = el.clone();
    for _ in 0..5 {
        if current.dataset().get("line").is_some() {
            return Some(current);
        }
        let parent = current.parent_element()?;
        current = parent.dyn_into::<web_sys::HtmlElement>().ok()?;
    }
    None
}

/// Handle arrow/movement key events.
fn handle_movement(buffer: &EditorBuffer, key: &str, ctrl: bool) {
    let (line, col) = buffer.cursor_pos();
    match key {
        "ArrowLeft" => {
            if col > 0 {
                buffer.set_cursor_pos(line, col - 1);
            } else if line > 0 {
                let prev_len = buffer.line_content(line - 1).trim_end().len();
                buffer.set_cursor_pos(line - 1, prev_len);
            }
        }
        "ArrowRight" => {
            let line_len = buffer.line_content(line).trim_end_matches('\n').len();
            if col < line_len {
                buffer.set_cursor_pos(line, col + 1);
            } else if line + 1 < buffer.num_lines() {
                buffer.set_cursor_pos(line + 1, 0);
            }
        }
        "ArrowUp" if line > 0 => buffer.set_cursor_pos(line - 1, col),
        "ArrowDown" if line + 1 < buffer.num_lines() => {
            buffer.set_cursor_pos(line + 1, col);
        }
        "Home" if ctrl => buffer.set_cursor_pos(0, 0),
        "Home" => buffer.set_cursor_pos(line, 0),
        "End" if ctrl => {
            let last = buffer.num_lines().saturating_sub(1);
            let len = buffer.line_content(last).trim_end_matches('\n').len();
            buffer.set_cursor_pos(last, len);
        }
        "End" => {
            let len = buffer.line_content(line).trim_end_matches('\n').len();
            buffer.set_cursor_pos(line, len);
        }
        "PageUp" => buffer.set_cursor_pos(line.saturating_sub(20), col),
        "PageDown" => {
            let target = (line + 20).min(buffer.num_lines().saturating_sub(1));
            buffer.set_cursor_pos(target, col);
        }
        _ => {}
    }
}

/// Notify the parent of the current cursor position (1-indexed).
fn notify_cursor(buffer: &EditorBuffer, on_cursor: &Callback<(u32, u32)>) {
    let (line, col) = buffer.cursor_pos();
    on_cursor.run(((line + 1) as u32, (col + 1) as u32));
}
