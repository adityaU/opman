//! PromptInput — rich text input with send/abort, slash commands, image attachments,
//! @mention popover, model/agent/memory chips, drag-and-drop, clipboard paste,
//! CLI-style history navigation (ArrowUp/Down), and Ctrl+R reverse-i-search.

mod consts;
mod handlers;
mod history;
mod parts;
mod search;
mod view;

use leptos::prelude::*;
use web_sys::HtmlTextAreaElement;

use crate::hooks::use_sse_state::{SessionStatus, SseState};
use crate::types::api::AgentInfo;
use consts::{ImageAttachmentLocal, truncate_preview};
use handlers::{detect_mention, execute_send, cursor_on_first_line, cursor_on_last_line};
use history::PromptHistory;

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn PromptInput(
    sse: SseState,
    #[prop(optional)] on_send: Option<Callback<(String, Option<Vec<String>>)>>,
    #[prop(optional)] on_command: Option<Callback<(String, String)>>,
    #[prop(optional)] on_abort: Option<Callback<()>>,
    #[prop(optional)] on_open_model_picker: Option<Callback<()>>,
    #[prop(optional)] on_open_agent_picker: Option<Callback<()>>,
    #[prop(optional)] on_open_memory: Option<Callback<()>>,
    #[prop(optional)] on_content_change: Option<Callback<bool>>,
    #[prop(optional)] current_model: Option<Signal<String>>,
    #[prop(optional)] current_agent: Option<Signal<String>>,
    #[prop(optional)] active_memory_labels: Option<Signal<Vec<String>>>,
) -> impl IntoView {
    // ── Reactive state ──────────────────────────────────────────────
    let (text, set_text) = signal(String::new());
    let (show_slash, set_show_slash) = signal(false);
    let (images, set_images) = signal::<Vec<ImageAttachmentLocal>>(Vec::new());
    let (drag_over, set_drag_over) = signal(false);
    let drag_counter = RwSignal::new(0i32);
    let (mention_filter, set_mention_filter) = signal::<Option<String>>(None);
    let (mention_agents, set_mention_agents) = signal::<Vec<AgentInfo>>(Vec::new());
    let (mentions, set_mentions) = signal::<Vec<String>>(Vec::new());
    let prompt_history = RwSignal::new(PromptHistory::new());
    let (rs_open, set_rs_open) = signal(false);
    let (rs_query, set_rs_query) = signal(String::new());
    let (rs_active_idx, set_rs_active_idx) = signal(0usize);
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // ── Derived signals ─────────────────────────────────────────────
    let session_status = sse.session_status;
    let tracked_sid = Memo::new(move |_| sse.tracked_session_id_reactive());
    let has_session = Memo::new(move |_| tracked_sid.get().is_some());
    let is_busy = Memo::new(move |_| session_status.get() == SessionStatus::Busy);
    let is_empty = Memo::new(move |_| text.get().trim().is_empty() && images.get().is_empty());

    let slash_filter = Memo::new(move |_| {
        let t = text.get();
        if t.starts_with('/') && !t.contains(' ') { t[1..].to_string() } else { String::new() }
    });

    let rs_matches = Signal::derive(move || {
        let q = rs_query.get();
        if q.is_empty() { return Vec::new(); }
        prompt_history.with_untracked(|h| {
            h.search(&q).into_iter().map(|(i, s)| (i, truncate_preview(s))).collect()
        })
    });

    // ── Reset transient state on session change ───────────────────
    {
        Effect::new(move |prev_sid: Option<Option<String>>| {
            let sid = tracked_sid.get();
            // Skip the very first run (mount) — only reset when session actually changes.
            if prev_sid.is_some() {
                set_text.set(String::new());
                set_show_slash.set(false);
                set_images.set(Vec::new());
                set_drag_over.set(false);
                drag_counter.set(0);
                set_mention_filter.set(None);
                set_mentions.set(Vec::new());
                set_rs_open.set(false);
                set_rs_query.set(String::new());
                set_rs_active_idx.set(0);
                prompt_history.update(|h| h.reset_nav());
            }
            sid
        });
    }

    // ── Populate history from messages ──────────────────────────────
    {
        let msgs = sse.messages;
        Effect::new(move |_| {
            let m = msgs.get();
            prompt_history.update(|h| h.rebuild(&m));
        });
    }

    // ── Fetch agents on mount ───────────────────────────────────────
    leptos::task::spawn_local(async move {
        if let Ok(agents) = crate::api::session::fetch_agents().await {
            set_mention_agents.set(agents);
        }
    });

    // ── Auto-resize textarea ────────────────────────────────────────
    Effect::new(move |_| {
        let t = text.get();
        let Some(el) = textarea_ref.get() else { return; };
        let ws_el: &web_sys::HtmlElement = &el;
        el.set_value(&t);
        let style = ws_el.style();
        style.set_property("height", "auto").ok();
        let sh = ws_el.scroll_height();
        let max_h = 200;
        style.set_property("height", &format!("{}px", sh.min(max_h))).ok();
        style.set_property("overflow-y", if sh > max_h { "auto" } else { "hidden" }).ok();
    });

    // ── Slash detection ─────────────────────────────────────────────
    Effect::new(move |_| {
        let t = text.get();
        let should = t.starts_with('/') && !t.contains(' ');
        if should != show_slash.get_untracked() { set_show_slash.set(should); }
    });

    // ── Content change notification ─────────────────────────────────
    {
        let on_cc = on_content_change.clone();
        Effect::new(move |_| {
            let empty = text.get().trim().is_empty() && images.get().is_empty();
            if let Some(ref cb) = on_cc { cb.run(!empty); }
        });
    }

    // ── Send handler ────────────────────────────────────────────────
    let on_send_clone = on_send.clone();
    let on_command_clone = on_command.clone();
    let handle_send = move || {
        let t = text.get();
        let m = mentions.get();
        let imgs = images.get();
        let trimmed = t.trim().to_string();
        if !trimmed.is_empty() && !trimmed.starts_with('/') {
            prompt_history.update(|h| h.push(&trimmed));
        }
        execute_send(&t, &m, &imgs, set_text, set_images, set_mentions, &on_send_clone, &on_command_clone);
    };

    let handle_abort = move || {
        if let Some(ref on_a) = on_abort { on_a.run(()); }
    };

    // ── Slash command handlers ──────────────────────────────────────
    let on_slash_select = {
        let on_cmd = on_command.clone();
        Callback::new(move |cmd_name: String| {
            if handlers::is_no_arg_command(&cmd_name) {
                if let Some(ref on_cmd) = on_cmd { on_cmd.run((cmd_name, String::new())); }
                set_text.set(String::new());
                set_show_slash.set(false);
            } else {
                set_text.set(format!("/{} ", cmd_name));
                set_show_slash.set(false);
            }
            if let Some(el) = textarea_ref.get() { let _ = el.focus(); }
        })
    };
    let on_slash_close = Callback::new(move |_: ()| { set_show_slash.set(false); });

    // ── @mention select ─────────────────────────────────────────────
    let handle_mention_select = move |agent_id: String| {
        if let Some(el) = textarea_ref.get() {
            let val = el.value();
            let sel_end = el.selection_end().ok().flatten().unwrap_or(0) as usize;
            let before = &val[..sel_end.min(val.len())];
            if let Some(at_pos) = before.rfind('@') {
                let after = &val[sel_end.min(val.len())..];
                set_text.set(format!("{}{}", &val[..at_pos], after));
            }
        }
        set_mentions.update(|m| { if !m.contains(&agent_id) { m.push(agent_id); } });
        set_mention_filter.set(None);
        if let Some(el) = textarea_ref.get() { let _ = el.focus(); }
    };

    let on_paste = handlers::make_paste_handler(set_images);
    let (on_dragenter, on_dragleave, on_dragover, on_drop) =
        handlers::make_drag_handlers(drag_counter, set_drag_over, set_images);
    let on_file_input_change = handlers::make_file_input_handler(file_input_ref, set_images);

    // ── Reverse-search handlers ─────────────────────────────────────
    let rs_on_query = Callback::new(move |q: String| { set_rs_query.set(q); set_rs_active_idx.set(0); });
    let rs_on_accept = Callback::new(move |orig_idx: usize| {
        if let Some(e) = prompt_history.with_untracked(|h| h.get(orig_idx).map(|s| s.to_owned())) {
            set_text.set(e);
        }
        set_rs_open.set(false); set_rs_query.set(String::new());
        if let Some(el) = textarea_ref.get() { let _ = el.focus(); }
    });
    let rs_on_close = Callback::new(move |_: ()| {
        set_rs_open.set(false); set_rs_query.set(String::new());
        if let Some(el) = textarea_ref.get() { let _ = el.focus(); }
    });
    let rs_on_next = Callback::new(move |_: ()| {
        let total = rs_matches.get_untracked().len();
        if total > 0 { set_rs_active_idx.update(|i| *i = (*i + 1) % total); }
    });

    // ── Keydown handler ─────────────────────────────────────────────
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.ctrl_key() && ev.key() == "r" {
            ev.prevent_default();
            if rs_open.get_untracked() { rs_on_next.run(()); } else {
                set_rs_open.set(true); set_rs_query.set(String::new()); set_rs_active_idx.set(0);
            }
            return;
        }
        if rs_open.get_untracked() { return; }
        if show_slash.get_untracked() {
            let key = ev.key();
            if key == "ArrowUp" || key == "ArrowDown" || key == "Tab" { return; }
            if key == "Escape" { ev.prevent_default(); set_show_slash.set(false); return; }
            if key == "Enter" { return; }
        }
        if mention_filter.get_untracked().is_some() && ev.key() == "Escape" {
            ev.prevent_default(); set_mention_filter.set(None); return;
        }
        if ev.key() == "ArrowUp" {
            if let Some(el) = textarea_ref.get() {
                let el_ref: &HtmlTextAreaElement = &el;
                if cursor_on_first_line(el_ref) {
                    let current = text.get_untracked();
                    if let Some((e, hc)) = prompt_history.with_untracked(|h| {
                        let mut hc = h.clone(); hc.prev(&current).map(|s| s.to_owned()).map(|e| (e, hc))
                    }) {
                        ev.prevent_default(); prompt_history.set(hc); set_text.set(e);
                    }
                }
            }
            return;
        }
        if ev.key() == "ArrowDown" {
            if let Some(el) = textarea_ref.get() {
                let el_ref: &HtmlTextAreaElement = &el;
                if cursor_on_last_line(el_ref) && prompt_history.with_untracked(|h| h.is_navigating()) {
                    if let Some((e, hc)) = prompt_history.with_untracked(|h| {
                        let mut hc = h.clone(); hc.next().map(|s| s.to_owned()).map(|e| (e, hc))
                    }) {
                        ev.prevent_default(); prompt_history.set(hc); set_text.set(e);
                    }
                }
            }
            return;
        }
        if ev.key() == "/" && text.get_untracked().is_empty() { set_show_slash.set(true); }
        if ev.key() == "Enter" && !ev.shift_key() { ev.prevent_default(); handle_send(); }
    };

    // ── Input handler ───────────────────────────────────────────────
    let on_input = move |ev: web_sys::Event| {
        set_text.set(event_target_value(&ev));
        if let Some(el) = textarea_ref.get() {
            let el_ref: &HtmlTextAreaElement = &el;
            detect_mention(el_ref, set_mention_filter);
        }
        prompt_history.update(|h| h.reset_nav());
    };

    // ── Render ──────────────────────────────────────────────────────
    view::prompt_view(view::PromptViewProps {
        drag_over, on_dragenter, on_dragleave, on_dragover, on_drop,
        show_slash, slash_filter, on_slash_select, on_slash_close,
        mention_filter, mention_agents, handle_mention_select,
        rs_open, rs_matches, rs_active_idx, rs_query,
        rs_on_query, rs_on_accept, rs_on_close, rs_on_next,
        current_model, current_agent, active_memory_labels,
        on_open_model_picker, on_open_agent_picker, on_open_memory,
        mentions, set_mentions, images, set_images,
        has_session, is_busy, is_empty,
        textarea_ref, file_input_ref,
        text, on_input, on_keydown, on_paste, on_file_input_change,
        handle_send, handle_abort,
    })
}
