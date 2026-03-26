//! Prompt sub-components — small view helpers extracted to keep mod.rs lean.
//! Each renders one visual section of the prompt input.

use super::consts::{short_model_name, ImageAttachmentLocal};
use super::search::ReverseSearchBar;
use crate::components::icons::*;
use crate::components::slash_command_popover::SlashCommandPopover;
use crate::types::api::AgentInfo;
use leptos::prelude::*;

// ── Selector chips ──────────────────────────────────────────────────

#[component]
pub fn SelectorChips(
    current_model: Option<Signal<String>>,
    current_agent: Option<Signal<String>>,
    active_memory_labels: Option<Signal<Vec<String>>>,
    on_open_model_picker: Option<Callback<()>>,
    on_open_agent_picker: Option<Callback<()>>,
    on_open_memory: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        {move || {
            let model_val = current_model.map(|s| s.get()).unwrap_or_default();
            let agent_val = current_agent.map(|s| s.get()).unwrap_or_default();
            let mem_labels = active_memory_labels.map(|s| s.get()).unwrap_or_default();
            let mem_count = mem_labels.len();
            let model_display = if model_val.is_empty() { "Model".to_string() } else { short_model_name(&model_val) };
            let on_model = on_open_model_picker.clone();
            let on_agent = on_open_agent_picker.clone();
            let on_memory = on_open_memory.clone();
            view! {
                <div class="prompt-selectors">
                    { let omc = on_model.clone(); view! {
                        <button class="prompt-chip" title="Change model"
                            on:click=move |_| { if let Some(ref cb) = omc { cb.run(()); } }
                        ><IconCpu size=12 /><span>{model_display.clone()}</span><IconChevronDown size=10 /></button>
                    }}
                    {(!agent_val.is_empty()).then(|| { let oac = on_agent.clone(); let ad = agent_val.clone(); view! {
                        <button class="prompt-chip" title="Change agent"
                            on:click=move |_| { if let Some(ref cb) = oac { cb.run(()); } }
                        ><span class="prompt-agent-dot"></span><span>{ad}</span><IconChevronDown size=10 /></button>
                    }})}
                    {(mem_count > 0).then(|| { let omc = on_memory.clone(); view! {
                        <button class="prompt-chip prompt-chip-memory" title="Open memory"
                            on:click=move |_| { if let Some(ref cb) = omc { cb.run(()); } }
                        ><IconBrain size=12 />{format!("{} {}", mem_count, if mem_count == 1 { "memory" } else { "memories" })}</button>
                    }})}
                </div>
            }
        }}
    }
}

// ── Mention popover ─────────────────────────────────────────────────

#[component]
pub fn MentionPopover(
    mention_filter: ReadSignal<Option<String>>,
    mention_agents: ReadSignal<Vec<AgentInfo>>,
    on_select: Callback<String>,
) -> impl IntoView {
    view! {
        {move || {
            let filter_text = mention_filter.get()?;
            let agents = mention_agents.get();
            let lf = filter_text.to_lowercase();
            let filtered: Vec<AgentInfo> = if lf.is_empty() { agents } else {
                agents.into_iter().filter(|a| a.id.to_lowercase().contains(&lf) || a.label.to_lowercase().contains(&lf)).collect()
            };
            if filtered.is_empty() { return None; }
            Some(view! {
                <div class="prompt-at-popover">
                    {filtered.into_iter().map(|agent| {
                        let aid = agent.id.clone(); let label = agent.label.clone();
                        let desc = agent.description.clone(); let aid_click = aid.clone();
                        let on_sel = on_select;
                        view! {
                            <button class="prompt-at-popover-item" on:click=move |_| on_sel.run(aid_click.clone())>
                                <IconAtSign size=14 />
                                <span class="prompt-at-name">{format!("@{}", aid)}</span>
                                <span class="prompt-at-label">{label}</span>
                                <span class="prompt-at-desc">{desc}</span>
                            </button>
                        }
                    }).collect_view()}
                </div>
            })
        }}
    }
}

// ── Mention pills ───────────────────────────────────────────────────

#[component]
pub fn MentionPills(
    mentions: ReadSignal<Vec<String>>,
    set_mentions: WriteSignal<Vec<String>>,
) -> impl IntoView {
    view! {
        {move || {
            let ms = mentions.get();
            (!ms.is_empty()).then(|| view! {
                <div class="prompt-agent-mentions">
                    {ms.iter().enumerate().map(|(idx, m)| { let md = m.clone(); view! {
                        <span class="prompt-agent-pill">
                            <IconAtSign size=12 /><span>{md}</span>
                            <button class="prompt-agent-pill-remove"
                                on:click=move |_| { set_mentions.update(|ms| { ms.remove(idx); }); }
                            ><IconX size=10 /></button>
                        </span>
                    }}).collect_view()}
                </div>
            })
        }}
    }
}

// ── Attachment previews ─────────────────────────────────────────────

#[component]
pub fn AttachmentPreviews(
    images: ReadSignal<Vec<ImageAttachmentLocal>>,
    set_images: WriteSignal<Vec<ImageAttachmentLocal>>,
) -> impl IntoView {
    view! {
        {move || {
            let imgs = images.get();
            (!imgs.is_empty()).then(|| view! {
                <div class="prompt-attachments">
                    {imgs.iter().enumerate().map(|(idx, img)| {
                        let src = img.data_url.clone(); let name = img.name.clone();
                        view! {
                            <div class="prompt-attachment-thumb">
                                <img src=src alt=name.clone() title=name.clone() />
                                <button class="prompt-attachment-remove"
                                    on:click=move |_| { set_images.update(|imgs| { imgs.remove(idx); }); }
                                ><IconX size=10 /></button>
                                <span class="prompt-attachment-name">{name}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            })
        }}
    }
}

// ── Hint bar ────────────────────────────────────────────────────────

#[component]
pub fn HintBar() -> impl IntoView {
    let is_mac = web_sys::window()
        .and_then(|w| w.navigator().platform().ok())
        .map(|p| p.to_lowercase().contains("mac"))
        .unwrap_or(false);
    let paste_key = if is_mac { "Cmd+V" } else { "Ctrl+V" };
    view! {
        <div class="prompt-hints">
            <span class="prompt-hint-key">"Enter"</span><span class="prompt-hint-label">"Send"</span>
            <span class="prompt-hint-key">"Shift+Enter"</span><span class="prompt-hint-label">"Newline"</span>
            <span class="prompt-hint-key">"/"</span><span class="prompt-hint-label">"Commands"</span>
            <span class="prompt-hint-key">{paste_key}</span><span class="prompt-hint-label">"Paste image"</span>
            <span class="prompt-hint-key">{"\u{2191}/\u{2193}"}</span><span class="prompt-hint-label">"History"</span>
            <span class="prompt-hint-key">"Ctrl+R"</span><span class="prompt-hint-label">"Search"</span>
        </div>
    }
}

// ── Prompt overlays (drag, slash, reverse-search) ───────────────────

#[component]
pub fn PromptOverlays(
    drag_over: ReadSignal<bool>,
    show_slash: ReadSignal<bool>,
    slash_filter: Memo<String>,
    on_slash_select: Callback<String>,
    on_slash_close: Callback<()>,
    rs_open: ReadSignal<bool>,
    rs_matches: Signal<Vec<(usize, String)>>,
    rs_active_idx: ReadSignal<usize>,
    rs_query: ReadSignal<String>,
    rs_on_query: Callback<String>,
    rs_on_accept: Callback<usize>,
    rs_on_close: Callback<()>,
    rs_on_next: Callback<()>,
) -> impl IntoView {
    view! {
        {move || drag_over.get().then(|| view! {
            <div class="prompt-drag-overlay"><IconImage size=24 /><span>"Drop image to attach"</span></div>
        })}
        {move || show_slash.get().then(|| view! {
            <SlashCommandPopover filter=Signal::derive(move || slash_filter.get()) on_select=on_slash_select on_close=on_slash_close />
        })}
        {move || rs_open.get().then(|| view! {
            <ReverseSearchBar matches=rs_matches active_idx=rs_active_idx query=rs_query
                on_query=rs_on_query on_accept=rs_on_accept on_close=rs_on_close on_next=rs_on_next />
        })}
    }
}
