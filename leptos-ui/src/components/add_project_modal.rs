//! AddProjectModal — browse filesystem and add project directories.
//! Matches React `AddProjectModal.tsx`.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::components::icons::*;
use crate::components::add_project_entry::AddProjectEntry;
use crate::hooks::use_sse_state::SseState;
use crate::types::api::{BrowseDirsResponse, DirEntry};

/// Add project modal component.
#[component]
pub fn AddProjectModal(
    on_close: Callback<()>,
) -> impl IntoView {
    let (browse_path, set_browse_path) = signal(String::new());
    let (browse_parent, set_browse_parent) = signal(String::new());
    let (browse_entries, set_browse_entries) = signal::<Vec<DirEntry>>(Vec::new());
    let (filter, set_filter) = signal(String::new());
    let (browse_loading, set_browse_loading) = signal(false);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (selected, set_selected) = signal(0usize);

    let filter_input_ref = NodeRef::<leptos::html::Input>::new();

    let browse_into = move |path: String| {
        set_browse_loading.set(true);
        set_filter.set(String::new());
        set_selected.set(0);
        set_error.set(String::new());
        leptos::task::spawn_local(async move {
            match crate::api::api_post::<BrowseDirsResponse>(
                "/dirs/browse",
                &serde_json::json!({ "path": path }),
            ).await {
                Ok(resp) => {
                    set_browse_path.set(resp.path);
                    set_browse_parent.set(resp.parent);
                    set_browse_entries.set(resp.entries);
                    set_browse_loading.set(false);
                }
                Err(e) => {
                    set_error.set(e.message);
                    set_browse_loading.set(false);
                }
            }
        });
    };

    // Load home dir on mount
    {
        let browse_into = browse_into.clone();
        leptos::task::spawn_local(async move {
            match crate::api::api_fetch::<serde_json::Value>("/dirs/home").await {
                Ok(val) => {
                    let path = val.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    browse_into(path);
                }
                Err(_) => browse_into(String::new()),
            }
        });
    }

    // Focus filter on path change
    Effect::new(move |_| {
        let _ = browse_path.get();
        if let Some(el) = filter_input_ref.get() {
            let _ = el.focus();
        }
    });

    let filtered = Memo::new(move |_| {
        let entries = browse_entries.get();
        let q = filter.get().to_lowercase();
        if q.is_empty() {
            return entries;
        }
        entries.into_iter().filter(|e| e.name.to_lowercase().contains(&q)).collect()
    });

    // Scroll selected into view
    Effect::new(move |_| {
        let _ = selected.get();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Ok(Some(el)) = doc.query_selector(".add-project-entry-selected") {
                if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
                    html_el.scroll_into_view();
                }
            }
        }
    });

    let browse_into2 = browse_into.clone();
    let browse_into3 = browse_into.clone();
    let browse_into4 = browse_into.clone();
    let browse_into5 = browse_into.clone();

    let sse = expect_context::<SseState>();

    let handle_add_current = move || {
        let path = browse_path.get_untracked();
        let set_app_state = sse.set_app_state;
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::api_post::<serde_json::Value>(
                "/project/add",
                &serde_json::json!({ "path": path }),
            ).await {
                Ok(_) => {
                    if let Ok(state) = crate::api::project::fetch_app_state().await {
                        set_app_state.set(Some(state));
                    }
                    on_close.run(());
                }
                Err(e) => {
                    set_error.set(e.message);
                    set_loading.set(false);
                }
            }
        });
    };

    let handle_add_entry = move |entry: DirEntry| {
        if entry.is_project {
            on_close.run(());
            return;
        }
        let path = entry.path.clone();
        let set_app_state = sse.set_app_state;
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::api_post::<serde_json::Value>(
                "/project/add",
                &serde_json::json!({ "path": path }),
            ).await {
                Ok(_) => {
                    if let Ok(state) = crate::api::project::fetch_app_state().await {
                        set_app_state.set(Some(state));
                    }
                    on_close.run(());
                }
                Err(e) => {
                    set_error.set(e.message);
                    set_loading.set(false);
                }
            }
        });
    };

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                let idx = selected.get_untracked();
                if let Some(entry) = items.get(idx) {
                    browse_into5(entry.path.clone());
                }
            }
            "Backspace" if filter.get_untracked().is_empty() => {
                let parent = browse_parent.get_untracked();
                if !parent.is_empty() {
                    browse_into5(parent);
                }
            }
            _ => {}
        }
    };

    let loading_sig = Signal::derive(move || loading.get());

    view! {
        <div class="add-project-overlay" on:click=move |_| on_close.run(())>
            <div class="add-project-modal" role="dialog" aria-modal="true"
                on:click=move |e| e.stop_propagation()
            >
                <div class="add-project-header">
                    <div class="add-project-header-left">
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>
                        </svg>
                        <h3>"Add Project"</h3>
                    </div>
                    <button class="add-project-close" on:click=move |_| on_close.run(()) title="Close (Esc)">
                        <IconX size=16 class="w-4 h-4" />
                    </button>
                </div>

                <div class="add-project-nav">
                    <button class="add-project-nav-btn"
                        on:click=move |_| {
                            let parent = browse_parent.get_untracked();
                            if !parent.is_empty() { browse_into2(parent); }
                        }
                        disabled=move || browse_parent.get().is_empty()
                        title="Go up (Backspace)"
                    >
                        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>
                        </svg>
                    </button>
                    <button class="add-project-nav-btn"
                        on:click=move |_| browse_into3(String::new()) title="Home"
                    >
                        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
                            <polyline points="9 22 9 12 15 12 15 22"/>
                        </svg>
                    </button>
                    <div class="add-project-path" title=move || browse_path.get()>
                        {move || browse_path.get()}
                    </div>
                    <button class="add-project-add-current"
                        on:click=move |_| handle_add_current()
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { "Adding..." } else { "Add This Directory" }}
                    </button>
                </div>

                <div class="add-project-search">
                    <IconSearch size=14 class="add-project-search-icon w-3.5 h-3.5" />
                    <input class="add-project-filter"
                        node_ref=filter_input_ref type="text"
                        placeholder="Filter directories..."
                        prop:value=move || filter.get()
                        on:input=move |e| {
                            set_filter.set(event_target_value(&e));
                            set_selected.set(0);
                        }
                        on:keydown=on_keydown
                        disabled=move || browse_loading.get()
                    />
                </div>

                <div class="add-project-body">
                    {move || {
                        if browse_loading.get() {
                            return view! { <div class="add-project-empty">"Loading..."</div> }.into_any();
                        }
                        let items = filtered.get();
                        let sel = selected.get();
                        if items.is_empty() {
                            return view! { <div class="add-project-empty">"No directories found"</div> }.into_any();
                        }
                        items.into_iter().enumerate().map(|(idx, entry)| {
                            let is_sel = idx == sel;
                            view! {
                                <AddProjectEntry
                                    entry=entry idx=idx is_selected=is_sel
                                    on_browse=Callback::new(move |p| browse_into4(p))
                                    on_add=Callback::new(move |e| handle_add_entry(e))
                                    on_hover=Callback::new(move |i| set_selected.set(i))
                                    loading=loading_sig
                                />
                            }
                        }).collect_view().into_any()
                    }}
                </div>

                {move || {
                    let err = error.get();
                    (!err.is_empty()).then(|| view! { <div class="add-project-error">{err}</div> })
                }}

                <div class="add-project-footer">
                    <span>"Click to browse, double-click or + to add"</span>
                    <span><kbd>"Backspace"</kbd>" Go up "<kbd>"Esc"</kbd>" Close"</span>
                </div>
            </div>
        </div>
    }
}
