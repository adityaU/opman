//! Cursor movement, DOM helpers, and line rendering for the native code editor.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::buffer_state::EditorBuffer;

/// Check if a key is a cursor movement key.
pub fn is_movement_key(key: &str) -> bool {
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

/// Handle arrow/movement key events.
pub fn handle_movement(buffer: &EditorBuffer, key: &str, ctrl: bool) {
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
        "ArrowDown" if line + 1 < buffer.num_lines() => buffer.set_cursor_pos(line + 1, col),
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

/// Walk up DOM to find the line container element.
pub fn find_line_element(el: &web_sys::HtmlElement) -> Option<web_sys::HtmlElement> {
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

/// Notify the parent of the current cursor position (1-indexed).
pub fn notify_cursor(buffer: &EditorBuffer, on_cursor: &Callback<(u32, u32)>) {
    let (line, col) = buffer.cursor_pos();
    on_cursor.run(((line + 1) as u32, (col + 1) as u32));
}

/// Render a single editor line (line number + highlighted spans + cursor).
pub fn render_line(
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
        <div class="native-editor-line" data-line=line_idx.to_string() style=bg>
            <span
                class="native-editor-gutter"
                style="display:inline-block;min-width:3.5em;padding-right:1em;\
                       text-align:right;color:var(--color-text-muted,#666);\
                       user-select:none;flex-shrink:0;"
            >{line_num.to_string()}</span>
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
