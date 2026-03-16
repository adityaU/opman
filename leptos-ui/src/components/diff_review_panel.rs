//! DiffReviewPanel — file diff review with accept/revert actions.
//! Matches React `diff-review-panel/DiffReviewPanel.tsx`.

use crate::api::client::{api_fetch, api_post};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{FileEditEntry, FileEditsResponse};
use leptos::prelude::*;
use std::collections::HashSet;

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) if idx > 0 => &path[..idx],
        _ => "",
    }
}

#[derive(Clone, PartialEq)]
enum FileStatus {
    Accepted,
    Reverted,
    Pending,
}

/// DiffReviewPanel component.
#[component]
pub fn DiffReviewPanel(
    on_close: Callback<()>,
    session_id: Option<String>,
    file_edit_count: ReadSignal<usize>,
) -> impl IntoView {
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);
    let (edits, set_edits) = signal(Vec::<FileEditEntry>::new());
    let (selected_file, set_selected_file) = signal(None::<String>);
    let (split_view, set_split_view) = signal(true);
    let (accepted_files, set_accepted_files) = signal(HashSet::<String>::new());
    let (reverted_files, set_reverted_files) = signal(HashSet::<String>::new());
    let (action_in_progress, set_action_in_progress) = signal(None::<String>);

    let sid = session_id.clone();
    let load_edits = move || {
        let sid = sid.clone();
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match &sid {
                Some(id) => {
                    match api_fetch::<FileEditsResponse>(&format!("/session/{}/file-edits", id))
                        .await
                    {
                        Ok(resp) => {
                            if !resp.edits.is_empty() && selected_file.get_untracked().is_none() {
                                set_selected_file.set(Some(resp.edits[0].path.clone()));
                            }
                            set_edits.set(resp.edits);
                            set_loading.set(false);
                        }
                        Err(e) => {
                            set_error.set(Some(format!("Failed to load file edits: {}", e)));
                            set_loading.set(false);
                        }
                    }
                }
                None => {
                    set_edits.set(vec![]);
                    set_loading.set(false);
                }
            }
        });
    };

    let load_init = load_edits.clone();
    load_init();

    // Auto-refresh when file_edit_count changes
    let load_on_change = load_edits.clone();
    Effect::new(move |_| {
        let _ = file_edit_count.get();
        load_on_change();
    });

    let load_refresh = load_edits.clone();

    let file_status = move |path: &str| -> FileStatus {
        if accepted_files.get().contains(path) {
            FileStatus::Accepted
        } else if reverted_files.get().contains(path) {
            FileStatus::Reverted
        } else {
            FileStatus::Pending
        }
    };

    let handle_accept = move |path: String| {
        set_action_in_progress.set(Some(path.clone()));
        set_accepted_files.update(|s| {
            s.insert(path.clone());
        });
        set_reverted_files.update(|s| {
            s.remove(&path);
        });
        set_action_in_progress.set(None);
    };

    let handle_revert = move |path: String| {
        let edits_snap = edits.get_untracked();
        let edit = edits_snap.iter().find(|e| e.path == path).cloned();
        if let Some(edit) = edit {
            set_action_in_progress.set(Some(path.clone()));
            let path2 = path.clone();
            leptos::task::spawn_local(async move {
                let body = serde_json::json!({
                    "path": edit.path,
                    "content": edit.original_content,
                });
                let _ = api_post::<serde_json::Value>(
                    "/file/write",
                    &body,
                )
                .await;
                set_reverted_files.update(|s| {
                    s.insert(path2.clone());
                });
                set_accepted_files.update(|s| {
                    s.remove(&path2);
                });
                set_action_in_progress.set(None);
            });
        }
    };

    let handle_accept_all = move |_: ()| {
        let paths: HashSet<String> = edits.get_untracked().iter().map(|e| e.path.clone()).collect();
        set_accepted_files.set(paths);
        set_reverted_files.set(HashSet::new());
    };

    let handle_revert_all = move |_: ()| {
        set_action_in_progress.set(Some("__all__".into()));
        let edits_snap = edits.get_untracked();
        leptos::task::spawn_local(async move {
            for edit in &edits_snap {
                let body = serde_json::json!({
                    "path": edit.path,
                    "content": edit.original_content,
                });
                let _ = api_post::<serde_json::Value>(
                    "/file/write",
                    &body,
                )
                .await;
            }
            let paths: HashSet<String> = edits_snap.iter().map(|e| e.path.clone()).collect();
            set_reverted_files.set(paths);
            set_accepted_files.set(HashSet::new());
            set_action_in_progress.set(None);
        });
    };

    view! {
        <ModalOverlay on_close=on_close class="diff-review-modal">
            // Header
            <div class="diff-review-header">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
                    <polyline points="14 2 14 8 20 8"/>
                </svg>
                <span class="diff-review-title">
                    "Diff Review"
                    {move || {
                        let count = edits.get().len();
                        if count > 0 {
                            Some(view! { <span class="diff-review-badge">{count}</span> })
                        } else { None }
                    }}
                </span>
                <div class="diff-review-header-actions">
                    <button class="diff-review-view-toggle" on:click=move |_| set_split_view.update(|v| *v = !*v)
                        title=move || if split_view.get() { "Unified view" } else { "Split view" }>
                        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            {move || if split_view.get() {
                                view! { <path d="M3 3h18v18H3zM3 12h18"/> }.into_any()
                            } else {
                                view! { <path d="M3 3h18v18H3zM12 3v18"/> }.into_any()
                            }}
                        </svg>
                    </button>
                    <button class="diff-review-refresh" on:click=move |_| load_refresh() title="Refresh">
                        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
                        </svg>
                    </button>
                    <button class="diff-review-close" on:click=move |_| on_close.run(()) title="Close (Esc)">
                        <IconX size=14 class="w-3.5 h-3.5" />
                    </button>
                </div>
            </div>

            // Body
            <div class="diff-review-body">
                {move || {
                    if loading.get() {
                        return view! {
                            <div class="diff-review-loading">
                                <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                <span>"Loading file edits..."</span>
                            </div>
                        }.into_any();
                    }
                    if let Some(err) = error.get() {
                        return view! { <div class="diff-review-error">{err}</div> }.into_any();
                    }
                    let edit_list = edits.get();
                    if edit_list.is_empty() {
                        return view! {
                            <div class="diff-review-empty">
                                <svg class="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
                                    <polyline points="14 2 14 8 20 8"/>
                                </svg>
                                <span>"No file edits in this session yet."</span>
                                <span class="diff-review-empty-hint">"File changes made by the AI will appear here for review."</span>
                            </div>
                        }.into_any();
                    }

                    // File list + diff area
                    let sel = selected_file.get();
                    let selected_edit = sel.as_ref().and_then(|s| edit_list.iter().find(|e| &e.path == s).cloned());
                    let pending_count = edit_list.iter().filter(|e| {
                        !accepted_files.get().contains(&e.path) && !reverted_files.get().contains(&e.path)
                    }).count();

                    view! {
                        <div class="diff-review-content">
                            // File list sidebar
                            <div class="diff-review-file-list">
                                <div class="diff-review-file-list-header">
                                    <span>{format!("Files ({})", edit_list.len())}</span>
                                    {if pending_count > 0 {
                                        Some(view! { <span class="diff-review-pending-badge">{format!("{} pending", pending_count)}</span> })
                                    } else { None }}
                                </div>
                                {edit_list.iter().map(|edit| {
                                    let path = edit.path.clone();
                                    let path2 = path.clone();
                                    let is_selected = sel.as_ref() == Some(&path);
                                    let status = file_status(&path);
                                    let status_cls = match status {
                                        FileStatus::Accepted => " file-accepted",
                                        FileStatus::Reverted => " file-reverted",
                                        FileStatus::Pending => "",
                                    };
                                    let cls = format!("diff-review-file-item{}{}", if is_selected { " selected" } else { "" }, status_cls);
                                    let fname = basename(&path).to_string();
                                    let dir = dirname(&path).to_string();
                                    view! {
                                        <button class=cls on:click=move |_| set_selected_file.set(Some(path2.clone()))>
                                            <span class="diff-review-file-indicator">
                                                {if is_selected {
                                                    view! { <IconChevronDown size=12 class="w-3 h-3" /> }.into_any()
                                                } else {
                                                    view! { <IconChevronRight size=12 class="w-3 h-3" /> }.into_any()
                                                }}
                                            </span>
                                            <span class="diff-review-file-name">{fname}</span>
                                            {match status {
                                                FileStatus::Accepted => Some(view! {
                                                    <span class="diff-review-file-status accepted">
                                                        <IconCheck size=10 class="w-2.5 h-2.5" />
                                                    </span>
                                                }.into_any()),
                                                FileStatus::Reverted => Some(view! {
                                                    <span class="diff-review-file-status reverted">
                                                        <svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7v6h6M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
                                                    </span>
                                                }.into_any()),
                                                _ => None,
                                            }}
                                            <span class="diff-review-file-dir">{dir}</span>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>

                            // Diff area
                            <div class="diff-review-diff-area">
                                {if let Some(edit) = selected_edit {
                                    let edit_path = edit.path.clone();
                                    let edit_path2 = edit.path.clone();
                                    let edit_path3 = edit.path.clone();
                                    let edit_path4 = edit.path.clone();
                                    view! {
                                        <div>
                                            <div class="diff-review-file-actions">
                                                <span class="diff-review-file-path">{edit.path.clone()}</span>
                                                <div class="diff-review-file-btns">
                                                    <button class="diff-review-btn accept"
                                                        on:click=move |_| handle_accept(edit_path.clone())
                                                        disabled=move || action_in_progress.get().is_some() || accepted_files.get().contains(&edit_path2)>
                                                        <IconCheck size=12 class="w-3 h-3" />
                                                        "Accept"
                                                    </button>
                                                    <button class="diff-review-btn revert"
                                                        on:click=move |_| handle_revert(edit_path3.clone())
                                                        disabled=move || action_in_progress.get().is_some() || reverted_files.get().contains(&edit_path4)>
                                                        <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7v6h6M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
                                                        "Revert"
                                                    </button>
                                                </div>
                                            </div>
                                            <div class="diff-review-viewer">
                                                <div class="diff-review-simple-diff">
                                                    <div class="diff-review-pane">
                                                        <div class="diff-review-pane-label">"Original"</div>
                                                        <pre class="diff-review-code diff-original">{edit.original_content.clone()}</pre>
                                                    </div>
                                                    <div class="diff-review-pane">
                                                        <div class="diff-review-pane-label">"Modified"</div>
                                                        <pre class="diff-review-code diff-modified">{edit.new_content.clone()}</pre>
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="diff-review-no-selection">"Select a file from the list to view changes."</div>
                                    }.into_any()
                                }}
                            </div>
                        </div>
                    }.into_any()
                }}
            </div>

            // Footer
            {move || {
                if !edits.get().is_empty() {
                    let pending = edits.get().iter().filter(|e| {
                        !accepted_files.get().contains(&e.path) && !reverted_files.get().contains(&e.path)
                    }).count();
                    Some(view! {
                        <div class="diff-review-footer">
                            <div class="diff-review-bulk-actions">
                                <button class="diff-review-btn accept-all"
                                    on:click=move |_| handle_accept_all(())
                                    disabled=move || action_in_progress.get().is_some() || pending == 0>
                                    <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 7 17l-5-5M22 10l-7.5 7.5L13 16"/></svg>
                                    "Accept All"
                                </button>
                                <button class="diff-review-btn revert-all"
                                    on:click=move |_| handle_revert_all(())
                                    disabled=move || action_in_progress.get().is_some() || pending == 0>
                                    <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7v6h6M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"/></svg>
                                    "Revert All"
                                </button>
                            </div>
                            <div class="diff-review-footer-hint">
                                <kbd>"Esc"</kbd>" to close"
                            </div>
                        </div>
                    })
                } else { None }
            }}
        </ModalOverlay>
    }
}
