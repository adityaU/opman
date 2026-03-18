//! Terminal panel body — renders tab content views and manages search state.

use leptos::prelude::*;
use send_wrapper::SendWrapper;

use super::native_term::{NativeTermView, TermScreen};
use super::pty_bridge::send_input;
use super::types::{kind_label, TabInfo, TabStatus};

/// Terminal body search state and callbacks.
pub struct SearchState {
    pub search_query: ReadSignal<String>,
    pub set_search_query: WriteSignal<String>,
    pub search_match_idx: ReadSignal<Option<usize>>,
    pub set_search_match_idx: WriteSignal<Option<usize>>,
    pub search_open: ReadSignal<bool>,
    pub set_search_open: WriteSignal<bool>,
}

impl SearchState {
    pub fn new() -> Self {
        let (search_query, set_search_query) = signal(String::new());
        let (search_match_idx, set_search_match_idx) = signal(Option::<usize>::None);
        let (search_open, set_search_open) = signal(false);
        Self {
            search_query,
            set_search_query,
            search_match_idx,
            set_search_match_idx,
            search_open,
            set_search_open,
        }
    }
}

/// Build search callbacks that operate on the active tab's screen.
pub fn build_search_callbacks(
    search: &SearchState,
    active_tab_id: ReadSignal<Option<String>>,
    tab_screens: StoredValue<
        std::collections::HashMap<
            String,
            (SendWrapper<TermScreen>, ReadSignal<u64>, WriteSignal<u64>),
        >,
    >,
) -> (
    Callback<String>, // search_change
    Callback<()>,     // search_next
    Callback<()>,     // search_prev
    Callback<()>,     // close_search
    Callback<()>,     // toggle_search
) {
    let set_search_query = search.set_search_query;
    let set_search_match_idx = search.set_search_match_idx;
    let search_query = search.search_query;
    let search_match_idx = search.search_match_idx;
    let search_open = search.search_open;
    let set_search_open = search.set_search_open;

    let count_matches = move |query: &str| -> usize {
        let screens = tab_screens.get_value();
        let Some(aid) = active_tab_id.get_untracked() else {
            return 0;
        };
        let Some((screen, _, _)) = screens.get(&aid) else {
            return 0;
        };
        screen.search(query).len()
    };

    let search_change = Callback::new(move |query: String| {
        set_search_query.set(query.clone());
        if query.is_empty() {
            set_search_match_idx.set(None);
        } else {
            let total = count_matches(&query);
            set_search_match_idx.set(if total > 0 { Some(0) } else { None });
        }
    });

    let search_next = Callback::new(move |_: ()| {
        let query = search_query.get_untracked();
        if query.is_empty() {
            return;
        }
        let total = count_matches(&query);
        if total == 0 {
            return;
        }
        let idx = search_match_idx.get_untracked().unwrap_or(0);
        set_search_match_idx.set(Some((idx + 1) % total));
    });

    let search_prev = Callback::new(move |_: ()| {
        let query = search_query.get_untracked();
        if query.is_empty() {
            return;
        }
        let total = count_matches(&query);
        if total == 0 {
            return;
        }
        let idx = search_match_idx.get_untracked().unwrap_or(0);
        set_search_match_idx.set(Some(if idx == 0 { total - 1 } else { idx - 1 }));
    });

    let close_search = Callback::new(move |_: ()| {
        set_search_open.set(false);
        set_search_query.set(String::new());
        set_search_match_idx.set(None);
    });

    let toggle_search = {
        let cs = close_search;
        Callback::new(move |_: ()| {
            if search_open.get_untracked() {
                cs.run(());
            } else {
                set_search_open.set(true);
            }
        })
    };

    (search_change, search_next, search_prev, close_search, toggle_search)
}

/// Render the terminal body — one `NativeTermView` per tab, visibility toggled.
pub fn render_terminal_body(
    tabs: ReadSignal<Vec<TabInfo>>,
    active_tab_id: ReadSignal<Option<String>>,
    tab_screens: StoredValue<
        std::collections::HashMap<
            String,
            (SendWrapper<TermScreen>, ReadSignal<u64>, WriteSignal<u64>),
        >,
    >,
    search_query: ReadSignal<String>,
    search_match_idx: ReadSignal<Option<usize>>,
) -> impl IntoView {
    view! {
        <div class="terminal-panel-body">
            {move || {
                let current_tabs = tabs.get();
                let active = active_tab_id.get();
                let screens = tab_screens.get_value();

                current_tabs.iter().map(|tab| {
                    let is_visible = active.as_deref() == Some(&tab.id);
                    let status = tab.status.clone();
                    let kind = tab.kind.clone();
                    let tab_id = tab.id.clone();
                    let tab_id_input = tab.id.clone();
                    let screen_data = screens.get(&tab.id).cloned();

                    view! {
                        <div
                            class="term-tab-body"
                            style:display=if is_visible { "flex" } else { "none" }
                            style="flex-direction:column;height:100%;"
                        >
                            {render_tab_status(status, &kind)}
                            {screen_data.map(|(screen, rev, set_rev)| {
                                render_tab_terminal(
                                    screen, rev, set_rev, tab_id, tab_id_input,
                                    search_query, search_match_idx,
                                )
                            })}
                        </div>
                    }
                }).collect::<Vec<_>>()
            }}
        </div>
    }
}

fn render_tab_status(status: TabStatus, kind: &str) -> Option<leptos::prelude::AnyView> {
    match status {
        TabStatus::Connecting => Some(
            view! {
                <div class="terminal-overlay">
                    {format!("Spawning {}...", kind_label(kind))}
                </div>
            }
            .into_any(),
        ),
        TabStatus::Error => Some(
            view! {
                <div class="terminal-overlay error">
                    {format!("Failed to start {}", kind_label(kind))}
                </div>
            }
            .into_any(),
        ),
        TabStatus::Ready => None,
    }
}

fn render_tab_terminal(
    screen: SendWrapper<TermScreen>,
    rev: ReadSignal<u64>,
    set_rev: WriteSignal<u64>,
    tab_id: String,
    tab_id_input: String,
    search_query: ReadSignal<String>,
    search_match_idx: ReadSignal<Option<usize>>,
) -> impl IntoView {
    let screen_resize = screen.clone();
    let tab_id_resize = tab_id.clone();

    let on_input = Callback::new(move |data: String| {
        send_input(&tab_id_input, data);
    });

    // Check for virtual-keyboard signal from context so we can suppress
    // resize events while the software keyboard is open (prevents the
    // "whole panel reloads" flash when tapping the terminal on mobile).
    let vkb_open: Option<ReadSignal<bool>> = use_context();

    let on_resize = Callback::new(move |(rows, cols): (u16, u16)| {
        // While the virtual keyboard is open the viewport shrinks
        // temporarily — resizing the PTY would flash / relayout lines
        // for a transient geometry, so we skip it entirely.
        if let Some(vkb) = vkb_open {
            if vkb.get_untracked() {
                return;
            }
        }
        let prev_rows = screen_resize.rows();
        let prev_cols = screen_resize.cols();
        if rows == prev_rows && cols == prev_cols {
            return;
        }
        screen_resize.resize(rows, cols);
        set_rev.update(|r| *r += 1);
        let pid = tab_id_resize.clone();
        leptos::task::spawn_local(async move {
            let _ = crate::api::pty::pty_resize(&pid, rows, cols).await;
        });
    });

    view! {
        <NativeTermView
            screen=screen
            revision=rev
            on_input=on_input
            on_resize=on_resize
            search_query=Signal::derive(move || search_query.get())
            search_active_idx=Signal::derive(move || search_match_idx.get())
        />
    }
}
