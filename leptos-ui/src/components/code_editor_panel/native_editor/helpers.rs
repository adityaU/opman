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

/// Imperatively update the cursor overlay in the DOM without re-rendering.
/// Moves the blinking cursor bar and line highlight to the current buffer
/// position by directly manipulating DOM element styles and classes.
pub fn update_cursor_in_container(
    container: &web_sys::HtmlElement,
    cur_line: usize,
    cur_col: usize,
) {
    let lines = match container.query_selector_all(".native-editor-line") {
        Ok(l) => l,
        Err(_) => return,
    };
    for i in 0..lines.length() {
        let Some(node) = lines.item(i) else { continue };
        let Ok(line_el) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let is_cur = line_el
            .dataset()
            .get("line")
            .and_then(|s| s.parse::<usize>().ok())
            == Some(cur_line);

        // Update cursor element visibility
        if let Ok(Some(cursor_el)) = line_el.query_selector(".native-editor-cursor") {
            if let Ok(cel) = cursor_el.dyn_into::<web_sys::HtmlElement>() {
                if is_cur {
                    cel.style().set_property("display", "").ok();
                    cel.style()
                        .set_property("left", &format!("{}ch", cur_col))
                        .ok();
                } else {
                    cel.style().set_property("display", "none").ok();
                }
            }
        }

        // Highlight current line
        if is_cur {
            line_el
                .style()
                .set_property("background", "rgba(255,255,255,0.04)")
                .ok();
        } else {
            line_el.style().set_property("background", "").ok();
        }
    }
}

/// Render a single editor line (line number + highlighted spans + cursor).
/// Every line gets a cursor element; it is hidden unless `is_cursor_line`.
/// This lets the imperative cursor updater move it between lines without
/// triggering a full reactive DOM re-render.
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
    let cursor_display = if is_cursor_line { "" } else { "display:none;" };
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
                <span
                    class="native-editor-cursor"
                    style=format!(
                        "position:absolute;left:{}ch;top:0;\
                         width:2px;height:1.5em;\
                         background:var(--color-text,#e0e0e0);\
                         animation:blink 1s step-end infinite;{}",
                        cursor_col, cursor_display
                    )
                />
            </span>
        </div>
    }
}
