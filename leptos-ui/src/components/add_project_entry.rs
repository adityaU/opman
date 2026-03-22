//! Single directory entry row for the AddProjectModal.

use crate::components::icons::*;
use crate::types::api::DirEntry;
use leptos::prelude::*;

/// A single browseable directory entry in the add-project modal.
#[component]
pub fn AddProjectEntry(
    entry: DirEntry,
    idx: usize,
    is_selected: bool,
    #[prop(into)] on_browse: Callback<String>,
    #[prop(into)] on_add: Callback<DirEntry>,
    #[prop(into)] on_hover: Callback<usize>,
    loading: Signal<bool>,
) -> impl IntoView {
    let is_existing = entry.is_project;
    let class_str = format!(
        "add-project-entry{}{}",
        if is_selected {
            " add-project-entry-selected"
        } else {
            ""
        },
        if is_existing {
            " add-project-entry-existing"
        } else {
            ""
        },
    );

    let browse_path = entry.path.clone();
    let title_path = entry.path.clone();
    let add_entry = entry.clone();
    let add_entry2 = entry.clone();
    let name = entry.name.clone();

    view! {
        <div
            class=class_str
            on:click=move |_| on_browse.run(browse_path.clone())
            on:dblclick=move |_| on_add.run(add_entry.clone())
            on:mouseenter=move |_| on_hover.run(idx)
            title=title_path
        >
            <svg
                class="add-project-entry-icon w-3.5 h-3.5"
                viewBox="0 0 24 24" fill="none" stroke="currentColor"
                stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            >
                <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>
            </svg>
            <span class="add-project-entry-name">{name}</span>
            {is_existing.then(|| view! {
                <svg
                    class="add-project-entry-star w-3 h-3"
                    viewBox="0 0 24 24" fill="currentColor" stroke="currentColor"
                    stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
                >
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                </svg>
            })}
            <button
                class="add-project-entry-add"
                on:click=move |e| {
                    e.stop_propagation();
                    on_add.run(add_entry2.clone());
                }
                title=if is_existing { "Already added" } else { "Add project" }
                disabled=move || loading.get()
            >
                {if is_existing {
                    view! {
                        <svg
                            class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor"
                            stroke="currentColor" stroke-width="2"
                            stroke-linecap="round" stroke-linejoin="round"
                        >
                            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                        </svg>
                    }.into_any()
                } else {
                    view! { <IconPlus size=14 class="w-3.5 h-3.5" /> }.into_any()
                }}
            </button>
        </div>
    }
}
