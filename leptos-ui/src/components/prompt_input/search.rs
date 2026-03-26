//! Reverse-i-search — Ctrl+R inline search bar for prompt history.
//! Renders a small bar above the textarea that filters previous user
//! messages in real-time as the user types, similar to bash's reverse-i-search.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

/// Props for the reverse-i-search bar.
#[component]
pub fn ReverseSearchBar(
    /// Matching entries: `(original_index, truncated_preview)`.
    matches: Signal<Vec<(usize, String)>>,
    /// Currently highlighted match index within `matches`.
    active_idx: ReadSignal<usize>,
    /// The current search query text (two-way via on_query).
    query: ReadSignal<String>,
    /// Called when the user types in the search input.
    on_query: Callback<String>,
    /// Called when the user accepts the current match (Enter or click).
    on_accept: Callback<usize>,
    /// Called to close the search bar (Escape).
    on_close: Callback<()>,
    /// Called to cycle to the next match (Ctrl+R again).
    on_next: Callback<()>,
) -> impl IntoView {
    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus the input on mount.
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        match key.as_str() {
            "Escape" => {
                ev.prevent_default();
                on_close.run(());
            }
            "Enter" => {
                ev.prevent_default();
                let ms = matches.get();
                let idx = active_idx.get();
                if let Some((orig, _)) = ms.get(idx) {
                    on_accept.run(*orig);
                } else {
                    on_close.run(());
                }
            }
            "r" if ev.ctrl_key() => {
                ev.prevent_default();
                on_next.run(());
            }
            _ => {}
        }
    };

    let on_input = move |ev: web_sys::Event| {
        let target = ev.target().unwrap().unchecked_into::<HtmlInputElement>();
        on_query.run(target.value());
    };

    let match_count = Signal::derive(move || matches.get().len());
    let active = active_idx;

    view! {
        <div class="prompt-reverse-search">
            <span class="prompt-rs-label">"(reverse-i-search)"</span>
            <input
                node_ref=input_ref
                class="prompt-rs-input"
                type="text"
                prop:value=query
                on:input=on_input
                on:keydown=on_keydown
                placeholder="type to search history..."
                spellcheck="false"
                autocomplete="off"
            />
            <span class="prompt-rs-count">
                {move || {
                    let total = match_count.get();
                    if total == 0 {
                        "no matches".to_string()
                    } else {
                        format!("{}/{}", active.get() + 1, total)
                    }
                }}
            </span>
            // Preview of matched entry
            {move || {
                let ms = matches.get();
                let idx = active.get();
                ms.get(idx).map(|(_, preview)| {
                    view! {
                        <span class="prompt-rs-preview" title=preview.clone()>
                            {preview.clone()}
                        </span>
                    }
                })
            }}
            <button
                class="prompt-rs-close"
                on:click=move |_| on_close.run(())
                title="Close (Esc)"
            >
                {"\u{2715}"}
            </button>
        </div>
    }
}
