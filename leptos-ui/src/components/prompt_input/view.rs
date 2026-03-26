//! Prompt input view — renders the full prompt UI using sub-components.
//! Separated from mod.rs to keep each file under 300 lines.

use leptos::prelude::*;

use super::parts::*;
use crate::components::icons::*;

// ── Props ───────────────────────────────────────────────────────────

pub struct PromptViewProps<
    FDE,
    FDL,
    FDO,
    FDR, // drag handlers
    FMS, // mention select
    FI,
    FK,
    FP,
    FFC, // input/key/paste/file
    FS: Clone + Send + Sync,
    FA: Clone + Send + Sync, // send/abort
> {
    pub drag_over: ReadSignal<bool>,
    pub on_dragenter: FDE,
    pub on_dragleave: FDL,
    pub on_dragover: FDO,
    pub on_drop: FDR,
    pub show_slash: ReadSignal<bool>,
    pub slash_filter: Memo<String>,
    pub on_slash_select: Callback<String>,
    pub on_slash_close: Callback<()>,
    pub mention_filter: ReadSignal<Option<String>>,
    pub mention_agents: ReadSignal<Vec<crate::types::api::AgentInfo>>,
    pub handle_mention_select: FMS,
    pub rs_open: ReadSignal<bool>,
    pub rs_matches: Signal<Vec<(usize, String)>>,
    pub rs_active_idx: ReadSignal<usize>,
    pub rs_query: ReadSignal<String>,
    pub rs_on_query: Callback<String>,
    pub rs_on_accept: Callback<usize>,
    pub rs_on_close: Callback<()>,
    pub rs_on_next: Callback<()>,
    pub current_model: Option<Signal<String>>,
    pub current_agent: Option<Signal<String>>,
    pub active_memory_labels: Option<Signal<Vec<String>>>,
    pub on_open_model_picker: Option<Callback<()>>,
    pub on_open_agent_picker: Option<Callback<()>>,
    pub on_open_memory: Option<Callback<()>>,
    pub mentions: ReadSignal<Vec<String>>,
    pub set_mentions: WriteSignal<Vec<String>>,
    pub images: ReadSignal<Vec<super::consts::ImageAttachmentLocal>>,
    pub set_images: WriteSignal<Vec<super::consts::ImageAttachmentLocal>>,
    pub has_session: Memo<bool>,
    pub is_busy: Memo<bool>,
    pub is_empty: Memo<bool>,
    pub textarea_ref: NodeRef<leptos::html::Textarea>,
    pub file_input_ref: NodeRef<leptos::html::Input>,
    pub text: ReadSignal<String>,
    pub on_input: FI,
    pub on_keydown: FK,
    pub on_paste: FP,
    pub on_file_input_change: FFC,
    pub handle_send: FS,
    pub handle_abort: FA,
}

// ── Render ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn prompt_view<
    FDE: Fn(web_sys::DragEvent) + 'static,
    FDL: Fn(web_sys::DragEvent) + 'static,
    FDO: Fn(web_sys::DragEvent) + 'static,
    FDR: Fn(web_sys::DragEvent) + 'static,
    FMS: Fn(String) + Send + Sync + 'static,
    FI: Fn(web_sys::Event) + 'static,
    FK: Fn(web_sys::KeyboardEvent) + 'static,
    FP: Fn(web_sys::ClipboardEvent) + 'static,
    FFC: Fn(web_sys::Event) + 'static,
    FS: Fn() + Clone + Send + Sync + 'static,
    FA: Fn() + Clone + Send + Sync + 'static,
>(
    p: PromptViewProps<FDE, FDL, FDO, FDR, FMS, FI, FK, FP, FFC, FS, FA>,
) -> impl IntoView {
    let send_for_btn = p.handle_send.clone();
    let abort_for_btn = p.handle_abort.clone();
    let has_session = p.has_session;
    let is_busy = p.is_busy;
    let is_empty = p.is_empty;
    let textarea_ref = p.textarea_ref;
    let file_input_ref = p.file_input_ref;

    view! {
        <div
            class="prompt-input-container"
            class=("prompt-drag-over", move || p.drag_over.get())
            on:dragenter=p.on_dragenter
            on:dragleave=p.on_dragleave
            on:dragover=p.on_dragover
            on:drop=p.on_drop
        >
            <PromptOverlays
                drag_over=p.drag_over
                show_slash=p.show_slash
                slash_filter=p.slash_filter
                on_slash_select=p.on_slash_select
                on_slash_close=p.on_slash_close
                rs_open=p.rs_open
                rs_matches=p.rs_matches
                rs_active_idx=p.rs_active_idx
                rs_query=p.rs_query
                rs_on_query=p.rs_on_query
                rs_on_accept=p.rs_on_accept
                rs_on_close=p.rs_on_close
                rs_on_next=p.rs_on_next
            />
            <MentionPopover
                mention_filter=p.mention_filter
                mention_agents=p.mention_agents
                on_select=Callback::new(p.handle_mention_select)
            />
            <div class="prompt-input-wrapper">
                <SelectorChips
                    current_model=p.current_model
                    current_agent=p.current_agent
                    active_memory_labels=p.active_memory_labels
                    on_open_model_picker=p.on_open_model_picker
                    on_open_agent_picker=p.on_open_agent_picker
                    on_open_memory=p.on_open_memory
                />
                <MentionPills mentions=p.mentions set_mentions=p.set_mentions />
                <AttachmentPreviews images=p.images set_images=p.set_images />
                // TextareaRow — inline to avoid generic #[component] issues
                <div class="prompt-textarea-row">
                    <button class="prompt-btn prompt-attach-btn" title="Attach image"
                        disabled=move || !has_session.get()
                        on:click=move |_| {
                            if let Some(el) = file_input_ref.get() {
                                let input: &web_sys::HtmlInputElement = &el;
                                input.click();
                            }
                        }
                    ><IconPaperclip size=16 /></button>
                    <input node_ref=file_input_ref type="file" multiple=true class="hidden"
                        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml,image/bmp"
                        on:change=p.on_file_input_change />
                    <textarea node_ref=textarea_ref class="prompt-textarea" rows=1
                        placeholder=move || {
                            if !has_session.get() { "Select or create a session to start..." }
                            else { "Type a message... (@ to mention, / for commands)" }
                        }
                        disabled=move || !has_session.get()
                        prop:value=p.text
                        on:input=p.on_input on:keydown=p.on_keydown on:paste=p.on_paste />
                    <div class="prompt-actions">
                        {move || is_busy.get().then(|| {
                            let abort = abort_for_btn.clone();
                            view! { <button class="prompt-btn prompt-abort-btn" title="Stop generation"
                                on:click=move |_| abort()><IconSquare size=16 /></button> }
                        })}
                        { let send = send_for_btn.clone(); view! {
                            <button class="prompt-btn prompt-send-btn"
                                class=("prompt-send-active", move || !is_empty.get())
                                disabled=move || is_empty.get() || !has_session.get()
                                title="Send message"
                                on:click=move |_| send()
                            ><IconSend size=16 /></button>
                        }}
                    </div>
                </div>
                <HintBar />
            </div>
        </div>
    }
}
