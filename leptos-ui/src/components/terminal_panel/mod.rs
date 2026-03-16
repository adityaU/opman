//! TerminalPanel — Rust-native terminal (vt100 + DOM). Multi-tab PTY management,
//! SSE streaming, search, resize, rename, expand/minimize. Matches React terminal-panel.

mod body;
mod header;
pub mod native_term;
mod pty_bridge;
mod render;
pub mod screen;
mod types;

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::*;

use body::{build_search_callbacks, render_terminal_body, SearchState};
use header::{render_header_actions, render_search_bar, render_tab_bar};
use native_term::TermScreen;
use pty_bridge::{init_tab, RuntimesHandle};
use types::{kind_label, make_uuid, TabInfo, TabStatus};

#[component]
pub fn TerminalPanel(
    #[prop(into, optional)] session_id: Option<Signal<Option<String>>>,
    #[prop(into, optional)] on_close: Option<Callback<()>>,
    #[prop(into, optional)] visible: Option<Signal<bool>>,
    #[prop(into, optional)] mcp_agent_active: Option<Signal<bool>>,
) -> impl IntoView {
    let (tabs, set_tabs) = signal(Vec::<TabInfo>::new());
    let (active_tab_id, set_active_tab_id) = signal(Option::<String>::None);
    let (expanded, set_expanded) = signal(false);
    let (kind_menu_open, set_kind_menu_open) = signal(false);
    let (rename_id, set_rename_id) = signal(Option::<String>::None);
    let (rename_value, set_rename_value) = signal(String::new());
    let tab_counter: StoredValue<u32> = StoredValue::new(0u32);
    let runtimes = RuntimesHandle::new();

    let tab_screens: StoredValue<
        std::collections::HashMap<String, (SendWrapper<TermScreen>, ReadSignal<u64>, WriteSignal<u64>)>,
    > = StoredValue::new(std::collections::HashMap::new());

    // ── Search state + callbacks ──
    let search = SearchState::new();
    let (search_change, search_next, search_prev, close_search, toggle_search) =
        build_search_callbacks(&search, active_tab_id, tab_screens);

    // ── Ctrl/Cmd+F shortcut — capture phase, toggles search ──
    {
        let toggle = toggle_search;
        let search_open = search.search_open;
        let document = web_sys::window().and_then(|w| w.document()).expect("no document");
        let cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if !(e.meta_key() || e.ctrl_key()) || e.key() != "f" { return; }
            let doc = web_sys::window().unwrap().document().unwrap();
            let Some(panel) = doc.query_selector(".terminal-panel").ok().flatten() else { return };
            let in_panel = doc.active_element()
                .map(|el| panel.contains(Some(&el)))
                .unwrap_or(false);
            if !in_panel && !search_open.get_untracked() { return; }
            e.prevent_default();
            e.stop_propagation();
            toggle.run(());
        });
        let _ = document.add_event_listener_with_callback_and_bool(
            "keydown", cb.as_ref().unchecked_ref(), true, // capture phase
        );
        let js_fn: js_sys::Function = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
        cb.forget();
        let doc_clone = document.clone();
        on_cleanup(move || {
            let _ = doc_clone.remove_event_listener_with_callback_and_bool("keydown", &js_fn, true);
        });
    }

    // ── create_tab callback ──
    let create_tab = {
        let sid = session_id;
        let runtimes = runtimes.clone();
        Callback::new(move |kind: String| {
            let count = tab_counter.get_value() + 1;
            tab_counter.set_value(count);
            let id = make_uuid();
            let label = format!("{} {}", kind_label(&kind), count);
            let tab = TabInfo { id: id.clone(), kind: kind.clone(), label, status: TabStatus::Connecting };
            set_tabs.update(|t| t.push(tab));
            set_active_tab_id.set(Some(id.clone()));
            set_kind_menu_open.set(false);

            let screen = SendWrapper::new(TermScreen::new(24, 80));
            let (rev, set_rev) = signal(0u64);
            tab_screens.update_value(|m| {
                m.insert(id.clone(), (screen.clone(), rev, set_rev));
            });

            let sid_val = sid.and_then(|s| s.get_untracked());
            init_tab(id, kind, sid_val, set_tabs, runtimes.inner().clone(), screen, set_rev);
        })
    };

    // Auto-create first tab
    {
        let ct = create_tab;
        Effect::new(move |prev: Option<()>| {
            if prev.is_none() { ct.run("shell".to_string()); }
        });
    }

    // ── close_tab ──
    let close_tab = {
        let runtimes = runtimes.clone();
        Callback::new(move |tab_id: String| {
            { let mut map = runtimes.borrow_mut();
              if let Some(mut runtime) = map.remove(&tab_id) { runtime.cleanup(); } }
            tab_screens.update_value(|m| { m.remove(&tab_id); });
            let pid = tab_id.clone();
            leptos::task::spawn_local(async move { let _ = crate::api::pty::pty_kill(&pid).await; });
            let tid = tab_id.clone();
            set_tabs.update(|ts| {
                let old_idx = ts.iter().position(|t| t.id == tid).unwrap_or(0);
                ts.retain(|t| t.id != tid);
                if active_tab_id.get_untracked().as_deref() == Some(&tid) {
                    if ts.is_empty() { set_active_tab_id.set(None); }
                    else { set_active_tab_id.set(Some(ts[old_idx.min(ts.len() - 1)].id.clone())); }
                }
            });
        })
    };

    // ── close_all ──
    let close_all = {
        let on_close = on_close;
        let runtimes = runtimes.clone();
        Callback::new(move |_: ()| {
            { let mut map = runtimes.borrow_mut();
              for (tab_id, mut runtime) in map.drain() {
                  runtime.cleanup();
                  let pid = tab_id;
                  leptos::task::spawn_local(async move { let _ = crate::api::pty::pty_kill(&pid).await; });
              } }
            tab_screens.update_value(|m| m.clear());
            set_tabs.set(Vec::new());
            set_active_tab_id.set(None);
            if let Some(cb) = on_close { cb.run(()); }
        })
    };

    // ── rename ──
    let start_rename = Callback::new(move |tab_id: String| {
        let label = tabs.get_untracked().iter()
            .find(|t| t.id == tab_id).map(|t| t.label.clone()).unwrap_or_default();
        set_rename_id.set(Some(tab_id));
        set_rename_value.set(label);
    });
    let commit_rename = Callback::new(move |_: ()| {
        if let Some(rid) = rename_id.get_untracked() {
            let val = rename_value.get_untracked();
            if !val.trim().is_empty() {
                set_tabs.update(|ts| {
                    if let Some(t) = ts.iter_mut().find(|t| t.id == rid) { t.label = val.trim().to_string(); }
                });
            }
        }
        set_rename_id.set(None);
        set_rename_value.set(String::new());
    });
    let cancel_rename = Callback::new(move |_: ()| {
        set_rename_id.set(None);
        set_rename_value.set(String::new());
    });

    // Close kind menu on outside click
    {
        let document = web_sys::window().and_then(|w| w.document()).expect("no document");
        let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |e: web_sys::Event| {
            if !kind_menu_open.get_untracked() { return; }
            let doc = web_sys::window().unwrap().document().unwrap();
            if let Some(wrapper) = doc.query_selector(".term-tab-new-wrapper").ok().flatten() {
                if let Some(target) = e.target() {
                    let node: web_sys::Node = target.unchecked_into();
                    if wrapper.contains(Some(&node)) { return; }
                }
            }
            set_kind_menu_open.set(false);
        });
        let _ = document.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
        let js_fn: js_sys::Function = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
        cb.forget();
        let doc_clone = document.clone();
        on_cleanup(move || {
            let _ = doc_clone.remove_event_listener_with_callback("mousedown", &js_fn);
        });
    }

    // Cleanup on unmount
    {
        let runtimes = runtimes.clone();
        on_cleanup(move || {
            let mut map = runtimes.borrow_mut();
            for (tab_id, mut runtime) in map.drain() {
                runtime.cleanup();
                let pid = tab_id;
                leptos::task::spawn_local(async move { let _ = crate::api::pty::pty_kill(&pid).await; });
            }
        });
    }

    // ── View ──
    let sq = search.search_query;
    let so = search.search_open;
    let smi = search.search_match_idx;

    view! {
        <div class=move || if expanded.get() { "terminal-panel expanded" } else { "terminal-panel" }>
            <div class="terminal-panel-header">
                {render_tab_bar(
                    tabs, active_tab_id, set_active_tab_id,
                    rename_id, rename_value, set_rename_value,
                    start_rename, commit_rename, cancel_rename,
                    close_tab, kind_menu_open, set_kind_menu_open, create_tab,
                )}
                {render_header_actions(
                    mcp_agent_active, so, toggle_search,
                    expanded, set_expanded, close_all,
                )}
            </div>

            {move || so.get().then(|| render_search_bar(
                sq, search_change, search_next, search_prev, close_search,
            ))}

            {render_terminal_body(tabs, active_tab_id, tab_screens, sq, smi)}
        </div>
    }
}
