//! Prompt input event handlers — send, slash, mention, paste, drag/drop, file,
//! keydown, and input handlers extracted from the main prompt_input component.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlTextAreaElement;

use super::consts::{ImageAttachmentLocal, ACCEPTED_IMAGE_TYPES, MAX_IMAGE_SIZE, NO_ARG_COMMANDS};
use leptos::prelude::*;

// ── Image file reading ──────────────────────────────────────────────

pub fn read_image_file(
    file: web_sys::File,
    set_images: leptos::prelude::WriteSignal<Vec<ImageAttachmentLocal>>,
) {
    let mime = file.type_();
    if !ACCEPTED_IMAGE_TYPES.iter().any(|t| *t == mime.as_str()) {
        log::warn!("Rejected file type: {}", mime);
        return;
    }
    let size = file.size() as usize;
    if size > MAX_IMAGE_SIZE {
        log::warn!("File too large: {} bytes (max {})", size, MAX_IMAGE_SIZE);
        return;
    }
    let name = file.name();
    let reader = match web_sys::FileReader::new() {
        Ok(r) => r,
        Err(_) => return,
    };
    let reader_clone = reader.clone();
    let closure = Closure::<dyn Fn()>::new(move || {
        if let Ok(result) = reader_clone.result() {
            if let Some(data_url) = result.as_string() {
                let attachment = ImageAttachmentLocal {
                    data_url,
                    name: name.clone(),
                    mime_type: mime.clone(),
                };
                set_images.update(|imgs| imgs.push(attachment));
            }
        }
    });
    reader.set_onload(Some(closure.as_ref().unchecked_ref()));
    closure.forget();
    let _ = reader.read_as_data_url(&file);
}

// ── @mention detection ──────────────────────────────────────────────

pub fn detect_mention(
    el: &HtmlTextAreaElement,
    set_mention_filter: leptos::prelude::WriteSignal<Option<String>>,
) {
    let val = el.value();
    let sel_end = el.selection_end().ok().flatten().unwrap_or(0) as usize;
    let before_cursor = &val[..sel_end.min(val.len())];
    if let Some(at_pos) = before_cursor.rfind('@') {
        let after_at = &before_cursor[at_pos + 1..];
        if !after_at.contains(' ') && !after_at.contains('\n') {
            set_mention_filter.set(Some(after_at.to_string()));
            return;
        }
    }
    set_mention_filter.set(None);
}

// ── Send logic ──────────────────────────────────────────────────────

/// Build the display text from trimmed input + mentions, clear state,
/// and delegate to on_send.
pub fn execute_send(
    text: &str,
    mentions: &[String],
    images: &[ImageAttachmentLocal],
    set_text: leptos::prelude::WriteSignal<String>,
    set_images: leptos::prelude::WriteSignal<Vec<ImageAttachmentLocal>>,
    set_mentions: leptos::prelude::WriteSignal<Vec<String>>,
    on_send: &Option<leptos::prelude::Callback<(String, Option<Vec<String>>)>>,
    on_command: &Option<leptos::prelude::Callback<(String, String)>>,
) {
    let trimmed = text.trim();
    if trimmed.is_empty() && images.is_empty() {
        return;
    }

    // Slash commands
    if trimmed.starts_with('/') {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd = parts[0].trim_start_matches('/').to_string();
        let args = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
        if let Some(ref on_cmd) = on_command {
            on_cmd.run((cmd, args));
        }
        set_text.set(String::new());
        return;
    }

    // Build display text with mentions
    let display_text = if mentions.is_empty() {
        trimmed.to_string()
    } else {
        let mention_str = mentions
            .iter()
            .map(|m| format!("@{}", m))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {}", mention_str, trimmed)
    };

    let image_data_urls: Option<Vec<String>> = if images.is_empty() {
        None
    } else {
        Some(images.iter().map(|i| i.data_url.clone()).collect())
    };

    // Clear state immediately for responsiveness
    set_text.set(String::new());
    set_images.set(Vec::new());
    set_mentions.set(Vec::new());

    if let Some(ref on_s) = on_send {
        on_s.run((display_text, image_data_urls));
    }
}

// ── Slash command helpers ───────────────────────────────────────────

/// Returns `true` if the command should execute immediately without args.
pub fn is_no_arg_command(cmd: &str) -> bool {
    NO_ARG_COMMANDS.contains(&cmd)
}

// ── Cursor position check ───────────────────────────────────────────

/// Returns `true` if the caret is on the first line of the textarea,
/// or the textarea value is empty.
pub fn cursor_on_first_line(el: &HtmlTextAreaElement) -> bool {
    let val = el.value();
    if val.is_empty() {
        return true;
    }
    let pos = el.selection_start().ok().flatten().unwrap_or(0) as usize;
    // First line = no newline before cursor position.
    !val[..pos.min(val.len())].contains('\n')
}

/// Returns `true` if the caret is on the last line of the textarea.
pub fn cursor_on_last_line(el: &HtmlTextAreaElement) -> bool {
    let val = el.value();
    if val.is_empty() {
        return true;
    }
    let pos = el.selection_end().ok().flatten().unwrap_or(0) as usize;
    !val[pos.min(val.len())..].contains('\n')
}

// ── Closure factories (keep mod.rs lean) ────────────────────────────

pub fn make_paste_handler(
    set_images: WriteSignal<Vec<ImageAttachmentLocal>>,
) -> impl Fn(web_sys::ClipboardEvent) + 'static {
    move |ev: web_sys::ClipboardEvent| {
        let Some(dt) = ev.clipboard_data() else {
            return;
        };
        let items = dt.items();
        for i in 0..items.length() {
            let Some(item) = items.get(i) else {
                continue;
            };
            if item.kind() == "file"
                && ACCEPTED_IMAGE_TYPES
                    .iter()
                    .any(|t| *t == item.type_().as_str())
            {
                if let Ok(Some(file)) = item.get_as_file() {
                    ev.prevent_default();
                    read_image_file(file, set_images);
                }
            }
        }
    }
}

pub fn make_drag_handlers(
    drag_counter: RwSignal<i32>,
    set_drag_over: WriteSignal<bool>,
    set_images: WriteSignal<Vec<ImageAttachmentLocal>>,
) -> (
    impl Fn(web_sys::DragEvent) + 'static,
    impl Fn(web_sys::DragEvent) + 'static,
    impl Fn(web_sys::DragEvent) + 'static,
    impl Fn(web_sys::DragEvent) + 'static,
) {
    let enter = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.update(|c| *c += 1);
        if let Some(dt) = ev.data_transfer() {
            let types = dt.types();
            if (0..types.length()).any(|i| {
                types
                    .get(i)
                    .as_string()
                    .map(|s| s == "Files")
                    .unwrap_or(false)
            }) {
                set_drag_over.set(true);
            }
        }
    };
    let leave = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.update(|c| *c -= 1);
        if drag_counter.get_untracked() <= 0 {
            drag_counter.set(0);
            set_drag_over.set(false);
        }
    };
    let over = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
    };
    let drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.set(0);
        set_drag_over.set(false);
        if let Some(dt) = ev.data_transfer() {
            if let Some(fl) = dt.files() {
                for i in 0..fl.length() {
                    if let Some(f) = fl.get(i) {
                        read_image_file(f, set_images);
                    }
                }
            }
        }
    };
    (enter, leave, over, drop)
}

pub fn make_file_input_handler(
    file_input_ref: NodeRef<leptos::html::Input>,
    set_images: WriteSignal<Vec<ImageAttachmentLocal>>,
) -> impl Fn(web_sys::Event) + 'static {
    move |_ev: web_sys::Event| {
        let Some(el) = file_input_ref.get() else {
            return;
        };
        let input: &web_sys::HtmlInputElement = &el;
        if let Some(fl) = input.files() {
            for i in 0..fl.length() {
                if let Some(f) = fl.get(i) {
                    read_image_file(f, set_images);
                }
            }
        }
        input.set_value("");
    }
}
