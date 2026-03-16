//! ContextInputModal — multi-line text input for injecting context.
//! Matches React `ContextInputModal.tsx`.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Context input modal component.
#[component]
pub fn ContextInputModal(on_close: Callback<()>, on_submit: Callback<String>) -> impl IntoView {
    let (text, set_text) = signal(String::new());
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();

    // Focus textarea on mount
    Effect::new(move |_| {
        if let Some(el) = textarea_ref.get() {
            let _ = el.focus();
        }
    });

    // Auto-resize textarea
    let auto_resize = move || {
        if let Some(el) = textarea_ref.get() {
            if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
                let style = html_el.style();
                let _ = style.set_property("height", "auto");
                let scroll_h = html_el.scroll_height();
                let h = scroll_h.min(400);
                let _ = style.set_property("height", &format!("{}px", h));
            }
        }
    };

    let handle_submit = move || {
        let trimmed = text.get_untracked().trim().to_string();
        if !trimmed.is_empty() {
            on_submit.run(trimmed);
            on_close.run(());
        }
    };

    let handle_submit_clone = handle_submit.clone();

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        let meta = e.meta_key() || e.ctrl_key();

        if key == "Escape" {
            e.prevent_default();
            on_close.run(());
        } else if meta && (key == "d" || key == "D") {
            e.prevent_default();
            handle_submit_clone();
        } else if meta && key == "Enter" {
            e.prevent_default();
            handle_submit_clone();
        }
    };

    let can_submit = move || !text.get().trim().is_empty();

    view! {
        <ModalOverlay on_close=on_close class="context-input-modal">
            <div class="context-input-header">
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/>
                    <path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>
                </svg>
                <div class="context-input-titles">
                    <span class="context-input-title">"Context Input"</span>
                    <span class="context-input-subtitle">"Insert context for the AI session"</span>
                </div>
                <button class="context-input-close" on:click=move |_| on_close.run(())>
                    <IconX size=14 class="w-3.5 h-3.5" />
                </button>
            </div>
            <div class="context-input-body">
                <textarea
                    class="context-input-textarea"
                    node_ref=textarea_ref
                    prop:value=move || text.get()
                    on:input=move |e| {
                        set_text.set(event_target_value(&e));
                        auto_resize();
                    }
                    on:keydown=on_keydown
                    placeholder="Paste context, instructions, or reference material..."
                    rows=8
                />
            </div>
            <div class="context-input-footer">
                <div class="context-input-hints">
                    <kbd>"Enter"</kbd>" Newline "
                    <kbd>"Cmd+Enter"</kbd>" Submit "
                    <kbd>"Cmd+D"</kbd>" Submit "
                    <kbd>"Esc"</kbd>" Cancel"
                </div>
                <button
                    class="context-input-submit"
                    on:click=move |_| handle_submit()
                    disabled=move || !can_submit()
                >
                    "Send Context"
                </button>
            </div>
        </ModalOverlay>
    }
}
