//! PromptInput — rich text input with send/abort, slash commands, image attachments,
//! @mention popover, model/agent/memory chips, drag-and-drop, and clipboard paste.
//! Matches React `web-ui/src/prompt-input/PromptInput.tsx` + `components.tsx` exactly.

use leptos::prelude::*;
use web_sys::HtmlTextAreaElement;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::*;
use crate::components::slash_command_popover::SlashCommandPopover;
use crate::hooks::use_sse_state::{SessionStatus, SseState};
use crate::types::api::AgentInfo;

// ── Constants ───────────────────────────────────────────────────────

const ACCEPTED_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "image/bmp",
];

const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// Commands that execute immediately without arguments (React NO_ARG_COMMANDS).
const NO_ARG_COMMANDS: &[&str] = &[
    "new", "cancel", "compact", "undo", "redo", "share", "fork",
    "terminal", "clear", "models", "keys", "keybindings", "todos",
    "sessions", "context", "settings", "assistant-center", "inbox",
    "missions", "memory", "autonomy", "routines", "delegation",
    "workspaces", "system",
];

// ── Helpers ─────────────────────────────────────────────────────────

/// Truncate a model ID to a short display name (React `shortModelName`).
fn short_model_name(model_id: &str) -> String {
    let parts: Vec<&str> = model_id.split('/').collect();
    let name = parts.last().unwrap_or(&model_id);
    if name.len() > 30 {
        format!("{}...", &name[..28])
    } else {
        name.to_string()
    }
}

/// Stored image attachment (base64 data-URL + metadata).
#[derive(Clone, Debug)]
struct ImageAttachmentLocal {
    data_url: String,
    name: String,
    mime_type: String,
}

// ── Component ───────────────────────────────────────────────────────

/// PromptInput component — the message input area.
/// Matches React `PromptInput` + sub-components from `components.tsx`.
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

    // Slash command popover
    let (show_slash, set_show_slash) = signal(false);

    // Image attachments
    let (images, set_images) = signal::<Vec<ImageAttachmentLocal>>(Vec::new());
    let (drag_over, set_drag_over) = signal(false);
    let drag_counter = RwSignal::new(0i32);

    // @mention popover
    let (mention_filter, set_mention_filter) = signal::<Option<String>>(None);
    let (mention_agents, set_mention_agents) = signal::<Vec<AgentInfo>>(Vec::new());
    let (mentions, set_mentions) = signal::<Vec<String>>(Vec::new());

    // Refs
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // ── Derived signals ─────────────────────────────────────────────
    let session_status = sse.session_status;
    let tracked_sid = Memo::new(move |_| sse.tracked_session_id_reactive());
    let has_session = Memo::new(move |_| tracked_sid.get().is_some());
    let is_busy = Memo::new(move |_| session_status.get() == SessionStatus::Busy);
    let is_empty = Memo::new(move |_| {
        text.get().trim().is_empty() && images.get().is_empty()
    });

    // Slash filter: derived from text
    let slash_filter = Memo::new(move |_| {
        let t = text.get();
        if t.starts_with('/') && !t.contains(' ') {
            t[1..].to_string()
        } else {
            String::new()
        }
    });

    // ── Fetch agents on mount (for @mention) ────────────────────────
    {
        leptos::task::spawn_local(async move {
            match crate::api::session::fetch_agents().await {
                Ok(agents) => set_mention_agents.set(agents),
                Err(_) => {}
            }
        });
    }

    // ── Auto-resize textarea ────────────────────────────────────────
    Effect::new(move |_| {
        let _t = text.get();
        if let Some(el) = textarea_ref.get() {
            let ws_el: &web_sys::HtmlElement = &el;
            let style = ws_el.style();
            style.set_property("height", "auto").ok();
            let scroll_height = ws_el.scroll_height();
            let max_height = 200;
            let new_height = scroll_height.min(max_height);
            style.set_property("height", &format!("{}px", new_height)).ok();
            // Enable scrolling once content exceeds max-height
            if scroll_height > max_height {
                style.set_property("overflow-y", "auto").ok();
            } else {
                style.set_property("overflow-y", "hidden").ok();
            }
        }
    });

    // ── Slash command detection (matches React onChange + onKeyDown) ──
    Effect::new(move |_| {
        let t = text.get();
        let should_show = t.starts_with('/') && !t.contains(' ');
        if should_show != show_slash.get_untracked() {
            set_show_slash.set(should_show);
        }
    });

    // ── Content change notification ─────────────────────────────────
    {
        let on_cc = on_content_change.clone();
        Effect::new(move |_| {
            let empty = text.get().trim().is_empty() && images.get().is_empty();
            if let Some(ref cb) = on_cc {
                cb.run(!empty);
            }
        });
    }

    // ── Image file reading ──────────────────────────────────────────

    fn read_image_file(file: web_sys::File, set_images: WriteSignal<Vec<ImageAttachmentLocal>>) {
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

    // ── Send logic ──────────────────────────────────────────────────

    let handle_send = move || {
        let trimmed = text.get().trim().to_string();
        let current_images = images.get();
        if trimmed.is_empty() && current_images.is_empty() {
            return;
        }

        // Check for slash commands
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

        // Build display text (include mentions)
        let current_mentions = mentions.get();
        let display_text = if current_mentions.is_empty() {
            trimmed.clone()
        } else {
            let mention_str = current_mentions
                .iter()
                .map(|m| format!("@{}", m))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} {}", mention_str, trimmed)
        };

        // Clear local input state immediately for responsiveness
        set_text.set(String::new());
        set_images.set(Vec::new());
        set_mentions.set(Vec::new());

        let image_data_urls: Option<Vec<String>> = if current_images.is_empty() {
            None
        } else {
            Some(current_images.iter().map(|i| i.data_url.clone()).collect())
        };

        // Delegate entirely to on_send (chat handler) which handles:
        // - optimistic message insertion
        // - memory injection
        // - API send
        // Do NOT add optimistic message or call API here to avoid duplicates.
        if let Some(ref on_s) = on_send {
            on_s.run((display_text, image_data_urls));
        }
    };

    let handle_abort = move || {
        if let Some(ref on_a) = on_abort {
            on_a.run(());
        }
        if let Some(sid) = sse.tracked_session_id() {
            leptos::task::spawn_local(async move {
                if let Err(e) = crate::api::abort_session(&sid).await {
                    log::error!("Failed to abort session: {}", e);
                }
            });
        }
    };

    // ── Slash command handlers ───────────────────────────────────────

    let on_slash_select = Callback::new(move |cmd_name: String| {
        // React: if NO_ARG_COMMANDS, execute immediately
        if NO_ARG_COMMANDS.contains(&cmd_name.as_str()) {
            if let Some(ref on_cmd) = on_command {
                on_cmd.run((cmd_name, String::new()));
            }
            set_text.set(String::new());
            set_show_slash.set(false);
        } else {
            // Otherwise insert /{command} into input and focus
            set_text.set(format!("/{} ", cmd_name));
            set_show_slash.set(false);
        }
        if let Some(el) = textarea_ref.get() {
            let _ = el.focus();
        }
    });

    let on_slash_close = Callback::new(move |_: ()| {
        set_show_slash.set(false);
    });

    // ── @mention handlers ───────────────────────────────────────────

    fn detect_mention(
        el: &HtmlTextAreaElement,
        set_mention_filter: WriteSignal<Option<String>>,
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

    let handle_mention_select = move |agent_id: String| {
        if let Some(el) = textarea_ref.get() {
            let val = el.value();
            let sel_end = el.selection_end().ok().flatten().unwrap_or(0) as usize;
            let before_cursor = &val[..sel_end.min(val.len())];
            if let Some(at_pos) = before_cursor.rfind('@') {
                let after_cursor = &val[sel_end.min(val.len())..];
                let new_val = format!("{}{}", &val[..at_pos], after_cursor);
                set_text.set(new_val);
            }
        }
        set_mentions.update(|m| {
            if !m.contains(&agent_id) {
                m.push(agent_id);
            }
        });
        set_mention_filter.set(None);
        if let Some(el) = textarea_ref.get() {
            let _ = el.focus();
        }
    };

    // ── Paste handler ───────────────────────────────────────────────

    let on_paste = move |ev: web_sys::ClipboardEvent| {
        if let Some(dt) = ev.clipboard_data() {
            let items = dt.items();
            let len = items.length();
            for i in 0..len {
                if let Some(item) = items.get(i) {
                    let kind = item.kind();
                    let item_type = item.type_();
                    if kind == "file"
                        && ACCEPTED_IMAGE_TYPES
                            .iter()
                            .any(|t| *t == item_type.as_str())
                    {
                        if let Ok(Some(file)) = item.get_as_file() {
                            ev.prevent_default();
                            read_image_file(file, set_images);
                        }
                    }
                }
            }
        }
    };

    // ── Drag/drop handlers ──────────────────────────────────────────

    let on_dragenter = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.update(|c| *c += 1);
        // Only show overlay when dragging files (React: dataTransfer.types.includes("Files"))
        if let Some(dt) = ev.data_transfer() {
            let types = dt.types();
            let has_files = (0..types.length()).any(|i| {
                types.get(i).as_string().map(|s| s == "Files").unwrap_or(false)
            });
            if has_files {
                set_drag_over.set(true);
            }
        }
    };

    let on_dragleave = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.update(|c| *c -= 1);
        if drag_counter.get_untracked() <= 0 {
            drag_counter.set(0);
            set_drag_over.set(false);
        }
    };

    let on_dragover = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
    };

    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_counter.set(0);
        set_drag_over.set(false);
        if let Some(dt) = ev.data_transfer() {
            if let Some(file_list) = dt.files() {
                let len = file_list.length();
                for i in 0..len {
                    if let Some(file) = file_list.get(i) {
                        read_image_file(file, set_images);
                    }
                }
            }
        }
    };

    // ── File input change handler ───────────────────────────────────

    let on_file_input_change = move |_ev: web_sys::Event| {
        if let Some(el) = file_input_ref.get() {
            let input: &web_sys::HtmlInputElement = &el;
            if let Some(file_list) = input.files() {
                let len = file_list.length();
                for i in 0..len {
                    if let Some(file) = file_list.get(i) {
                        read_image_file(file, set_images);
                    }
                }
            }
            input.set_value("");
        }
    };

    // ── Keydown handler ─────────────────────────────────────────────

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        // Let slash popover handle its own keys when open
        if show_slash.get_untracked() {
            let key = ev.key();
            if key == "ArrowUp" || key == "ArrowDown" || key == "Tab" {
                return;
            }
            if key == "Escape" {
                ev.prevent_default();
                set_show_slash.set(false);
                return;
            }
            if key == "Enter" {
                return;
            }
        }

        // Close mention popover on Escape
        if mention_filter.get_untracked().is_some() && ev.key() == "Escape" {
            ev.prevent_default();
            set_mention_filter.set(None);
            return;
        }

        // React: "/" at empty input opens slash popover
        if ev.key() == "/" && text.get_untracked().is_empty() {
            set_show_slash.set(true);
        }

        if ev.key() == "Enter" && !ev.shift_key() && !ev.ctrl_key() && !ev.meta_key() {
            ev.prevent_default();
            // Allow sending even when busy — user explicitly wants to queue messages
            handle_send();
        }
    };

    // ── Input handler ───────────────────────────────────────────────

    let on_input = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        set_text.set(val);
        if let Some(el) = textarea_ref.get() {
            let el_ref: &HtmlTextAreaElement = &el;
            detect_mention(el_ref, set_mention_filter);
        }
    };

    // ── View ────────────────────────────────────────────────────────
    // Structure matches React exactly:
    //   div.prompt-input-container
    //     <DragOverlay />         (conditional)
    //     <SlashCommandPopover /> (conditional)
    //     <AtMentionPopover />    (conditional)
    //     div.prompt-input-wrapper
    //       <SelectorChips />
    //       <AgentMentionPills />
    //       <AttachmentPreviews />
    //       <TextareaRow />
    //       <HintBar />

    view! {
        <div
            class="prompt-input-container"
            class=("prompt-drag-over", move || drag_over.get())
            on:dragenter=on_dragenter
            on:dragleave=on_dragleave
            on:dragover=on_dragover
            on:drop=on_drop
        >
            // DragOverlay (React: <DragOverlay />)
            {move || {
                if drag_over.get() {
                    Some(view! {
                        <div class="prompt-drag-overlay">
                            <IconImage size=24 />
                            <span>"Drop image to attach"</span>
                        </div>
                    }.into_any())
                } else {
                    None
                }
            }}

            // SlashCommandPopover (React: <SlashCommandPopover />)
            {move || {
                if show_slash.get() {
                    Some(view! {
                        <SlashCommandPopover
                            filter=Signal::derive(move || slash_filter.get())
                            on_select=on_slash_select
                            on_close=on_slash_close
                        />
                    }.into_any())
                } else {
                    None
                }
            }}

            // AtMentionPopover (React: <AtMentionPopover />)
            {move || {
                let mf = mention_filter.get();
                if let Some(filter_text) = mf {
                    let agents = mention_agents.get();
                    let lf = filter_text.to_lowercase();
                    let filtered: Vec<AgentInfo> = if lf.is_empty() {
                        agents
                    } else {
                        agents
                            .into_iter()
                            .filter(|a| {
                                a.id.to_lowercase().contains(&lf)
                                    || a.label.to_lowercase().contains(&lf)
                            })
                            .collect()
                    };
                    if filtered.is_empty() {
                        None
                    } else {
                        Some(view! {
                            <div class="prompt-at-popover">
                                {filtered.into_iter().map(|agent| {
                                    let aid = agent.id.clone();
                                    let label = agent.label.clone();
                                    let desc = agent.description.clone();
                                    let aid_click = aid.clone();
                                    view! {
                                        <button
                                            class="prompt-at-popover-item"
                                            on:click=move |_| {
                                                handle_mention_select(aid_click.clone());
                                            }
                                        >
                                            // AtSign icon
                                            <IconAtSign size=14 />
                                            <span class="prompt-at-name">{format!("@{}", aid)}</span>
                                            <span class="prompt-at-label">{label}</span>
                                            <span class="prompt-at-desc">{desc}</span>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any())
                    }
                } else {
                    None
                }
            }}

            // Main wrapper (React: div.prompt-input-wrapper)
            <div class="prompt-input-wrapper">

                // SelectorChips (React: <SelectorChips />)
                {move || {
                    let model_val = current_model.map(|s| s.get()).unwrap_or_default();
                    let agent_val = current_agent.map(|s| s.get()).unwrap_or_default();
                    let mem_labels = active_memory_labels.map(|s| s.get()).unwrap_or_default();
                    let mem_count = mem_labels.len();

                    let has_chips = !model_val.is_empty()
                        || !agent_val.is_empty()
                        || mem_count > 0;

                    if !has_chips {
                        return None;
                    }

                    let model_display = if model_val.is_empty() {
                        String::new()
                    } else {
                        short_model_name(&model_val)
                    };
                    let agent_display = agent_val;

                    let on_model = on_open_model_picker.clone();
                    let on_agent = on_open_agent_picker.clone();
                    let on_memory = on_open_memory.clone();

                    Some(view! {
                        <div class="prompt-selectors">
                            // Model chip
                            {if !model_display.is_empty() {
                                let on_model_inner = on_model.clone();
                                Some(view! {
                                    <button
                                        class="prompt-chip"
                                        title="Change model"
                                        on:click=move |_| {
                                            if let Some(ref cb) = on_model_inner { cb.run(()); }
                                        }
                                    >
                                        <IconCpu size=12 />
                                        <span>{model_display}</span>
                                        <IconChevronDown size=10 />
                                    </button>
                                })
                            } else {
                                None
                            }}
                            // Agent chip
                            {if !agent_display.is_empty() {
                                let on_agent_inner = on_agent.clone();
                                let agent_disp = agent_display.clone();
                                Some(view! {
                                    <button
                                        class="prompt-chip"
                                        title="Change agent"
                                        on:click=move |_| {
                                            if let Some(ref cb) = on_agent_inner { cb.run(()); }
                                        }
                                    >
                                        <span class="prompt-agent-dot"></span>
                                        <span>{agent_disp}</span>
                                        <IconChevronDown size=10 />
                                    </button>
                                })
                            } else {
                                None
                            }}
                            // Memory chip
                            {if mem_count > 0 {
                                let on_memory_inner = on_memory.clone();
                                Some(view! {
                                    <button
                                        class="prompt-chip prompt-chip-memory"
                                        title="Open memory"
                                        on:click=move |_| {
                                            if let Some(ref cb) = on_memory_inner { cb.run(()); }
                                        }
                                    >
                                        <IconBrain size=12 />
                                        {format!("{} {}", mem_count, if mem_count == 1 { "memory" } else { "memories" })}
                                    </button>
                                })
                            } else {
                                None
                            }}
                        </div>
                    })
                }}

                // AgentMentionPills (React: <AgentMentionPills />)
                {move || {
                    let current_mentions = mentions.get();
                    if current_mentions.is_empty() {
                        None
                    } else {
                        Some(view! {
                            <div class="prompt-agent-mentions">
                                {current_mentions.iter().enumerate().map(|(idx, m)| {
                                    let m_display = m.clone();
                                    view! {
                                        <span class="prompt-agent-pill">
                                            // AtSign icon
                                            <IconAtSign size=12 />
                                            <span>{m_display}</span>
                                            <button
                                                class="prompt-agent-pill-remove"
                                                on:click=move |_| {
                                                    set_mentions.update(|ms| { ms.remove(idx); });
                                                }
                                            >
                                                // X icon
                                                <IconX size=10 />
                                            </button>
                                        </span>
                                    }
                                }).collect_view()}
                            </div>
                        })
                    }
                }}

                // AttachmentPreviews (React: <AttachmentPreviews />)
                {move || {
                    let current_images = images.get();
                    if current_images.is_empty() {
                        None
                    } else {
                        Some(view! {
                            <div class="prompt-attachments">
                                {current_images.iter().enumerate().map(|(idx, img)| {
                                    let src = img.data_url.clone();
                                    let name = img.name.clone();
                                    view! {
                                        <div class="prompt-attachment-thumb">
                                            <img
                                                src=src
                                                alt=name.clone()
                                                title=name.clone()
                                            />
                                            <button
                                                class="prompt-attachment-remove"
                                                on:click=move |_| {
                                                    set_images.update(|imgs| { imgs.remove(idx); });
                                                }
                                            >
                                                <IconX size=10 />
                                            </button>
                                            <span class="prompt-attachment-name">{name}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        })
                    }
                }}

                // TextareaRow (React: <TextareaRow />)
                <div class="prompt-textarea-row">
                    // Paperclip attach button (React: Paperclip icon)
                    <button
                        class="prompt-btn prompt-attach-btn"
                        title="Attach image"
                        disabled=move || !has_session.get()
                        on:click=move |_| {
                            if let Some(el) = file_input_ref.get() {
                                let input: &web_sys::HtmlInputElement = &el;
                                input.click();
                            }
                        }
                    >
                        // Paperclip icon (Lucide)
                        <IconPaperclip size=16 />
                    </button>

                    // Hidden file input
                    <input
                        node_ref=file_input_ref
                        type="file"
                        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
                        multiple=true
                        class="hidden"
                        on:change=on_file_input_change
                    />

                    // Textarea
                    <textarea
                        node_ref=textarea_ref
                        class="prompt-textarea"
                        placeholder=move || {
                            if !has_session.get() {
                                "Select or create a session to start..."
                            } else {
                                "Type a message... (@ to mention, / for commands)"
                            }
                        }
                        disabled=move || !has_session.get()
                        rows=1
                        prop:value=text
                        on:input=on_input
                        on:keydown=on_keydown
                        on:paste=on_paste
                    />

                    // Actions (React: div.prompt-actions)
                    <div class="prompt-actions">
                        // Abort button — shown only when busy
                        {move || {
                            if is_busy.get() {
                                Some(view! {
                                    <button
                                        class="prompt-btn prompt-abort-btn"
                                        title="Stop generation"
                                        on:click=move |_| handle_abort()
                                    >
                                        <IconSquare size=16 />
                                    </button>
                                })
                            } else {
                                None
                            }
                        }}
                        // Send button — always visible (disabled when empty / no session)
                        <button
                            class="prompt-btn prompt-send-btn"
                            class=("prompt-send-active", move || !is_empty.get())
                            disabled=move || is_empty.get() || !has_session.get()
                            title="Send message"
                            on:click=move |_| handle_send()
                        >
                            <IconSend size=16 />
                        </button>
                    </div>
                </div>

                // HintBar (React: <HintBar />)
                <div class="prompt-hints">
                    <span class="prompt-hint-key">"Enter"</span>
                    <span class="prompt-hint-label">"Send"</span>
                    <span class="prompt-hint-key">"Shift+Enter"</span>
                    <span class="prompt-hint-label">"Newline"</span>
                    <span class="prompt-hint-key">"/"</span>
                    <span class="prompt-hint-label">"Commands"</span>
                    <span class="prompt-hint-key">{
                        if web_sys::window()
                            .and_then(|w| w.navigator().platform().ok())
                            .map(|p| p.to_lowercase().contains("mac"))
                            .unwrap_or(false)
                        { "Cmd+V" } else { "Ctrl+V" }
                    }</span>
                    <span class="prompt-hint-label">"Paste image"</span>
                </div>
            </div>
        </div>
    }
}
