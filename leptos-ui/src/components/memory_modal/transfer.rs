//! Memory transfer — download/upload with scope selection.
//! Client-side JSON export via Blob URL, import via file input + existing create API.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::HtmlInputElement;
use crate::api::client::{api_fetch, api_post};
use crate::components::icons::*;
use crate::types::api::{PersonalMemoryItem, PersonalMemoryListResponse};
use super::helpers::CreateMemoryBody;

/// Portable memory format for JSON export/import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryExportItem {
    pub label: String,
    pub content: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryExportEnvelope {
    version: u8,
    exported_at: String,
    items: Vec<MemoryExportItem>,
}

fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
}

/// Panel shown inside the memory modal footer for download/upload.
#[component]
pub fn MemoryTransferBar(
    items: ReadSignal<Vec<PersonalMemoryItem>>,
    set_items: WriteSignal<Vec<PersonalMemoryItem>>,
    active_project_index: usize,
    active_session_id: Option<String>,
) -> impl IntoView {
    let (show_panel, set_show_panel) = signal(Option::<&'static str>::None); // "download" | "upload"
    let (dl_global, set_dl_global) = signal(true);
    let (dl_project, set_dl_project) = signal(true);
    let (dl_session, set_dl_session) = signal(true);
    let (ul_status, set_ul_status) = signal(String::new());
    let (busy, set_busy) = signal(false);

    let api = active_project_index;
    let sid = active_session_id.clone();

    // Download handler
    let handle_download = move |_: web_sys::MouseEvent| {
        let g = dl_global.get_untracked();
        let p = dl_project.get_untracked();
        let s = dl_session.get_untracked();
        if !g && !p && !s { return; }
        set_busy.set(true);
        leptos::task::spawn_local(async move {
            let result = api_fetch::<PersonalMemoryListResponse>("/memory").await;
            set_busy.set(false);
            let all = match result {
                Ok(r) => r.memory,
                Err(e) => {
                    leptos::logging::warn!("Download failed: {}", e);
                    return;
                }
            };
            let filtered: Vec<MemoryExportItem> = all.into_iter()
                .filter(|m| match m.scope.as_str() {
                    "global" => g,
                    "project" => p,
                    "session" => s,
                    _ => false,
                })
                .map(|m| MemoryExportItem {
                    label: m.label,
                    content: m.content,
                    scope: m.scope,
                    project_index: m.project_index,
                    session_id: m.session_id,
                })
                .collect();
            if filtered.is_empty() { return; }
            let envelope = MemoryExportEnvelope {
                version: 1,
                exported_at: now_iso(),
                items: filtered,
            };
            let json = match serde_json::to_string_pretty(&envelope) {
                Ok(j) => j,
                Err(_) => return,
            };
            trigger_download(&json, "opman-memory.json");
        });
    };

    // Upload handler
    let sid_ul = sid.clone();
    let handle_file = move |ev: web_sys::Event| {
        let input: HtmlInputElement = match ev.target() {
            Some(t) => match t.dyn_into() {
                Ok(i) => i,
                Err(_) => return,
            },
            None => return,
        };
        let files = match input.files() {
            Some(f) => f,
            None => return,
        };
        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };
        let sid_inner = sid_ul.clone();
        set_busy.set(true);
        set_ul_status.set("Reading file...".into());
        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(_) => return,
        };
        let reader_clone = reader.clone();
        let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let text = match reader_clone.result() {
                Ok(v) => match v.as_string() {
                    Some(s) => s,
                    None => return,
                },
                Err(_) => return,
            };
            let sid_spawn = sid_inner.clone();
            leptos::task::spawn_local(async move {
                let envelope: MemoryExportEnvelope = match serde_json::from_str(&text) {
                    Ok(e) => e,
                    Err(e) => {
                        set_ul_status.set(format!("Invalid JSON: {}", e));
                        set_busy.set(false);
                        return;
                    }
                };
                let total = envelope.items.len();
                let mut imported = 0usize;
                for item in envelope.items {
                    let body = CreateMemoryBody {
                        label: item.label,
                        content: item.content,
                        scope: item.scope.clone(),
                        project_index: if item.scope == "project" || item.scope == "session" {
                            item.project_index.or(Some(api))
                        } else {
                            None
                        },
                        session_id: if item.scope == "session" {
                            item.session_id.or_else(|| sid_spawn.clone())
                        } else {
                            None
                        },
                    };
                    if let Ok(created) = api_post::<PersonalMemoryItem>("/memory", &body).await {
                        set_items.update(|list| list.push(created));
                        imported += 1;
                    }
                    set_ul_status.set(format!("{}/{} imported", imported, total));
                }
                set_ul_status.set(format!("Done — {} imported", imported));
                set_busy.set(false);
            });
        }) as Box<dyn FnMut(_)>);
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
        let _ = reader.read_as_text(&file);
        // Reset input so the same file can be re-selected
        input.set_value("");
    };

    view! {
        <div class="memory-transfer-bar">
            // Trigger buttons (always visible)
            {move || {
                if show_panel.get().is_none() {
                    view! {
                        <div class="memory-transfer-triggers">
                            <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Enter"</kbd>" Edit "<kbd>"Esc"</kbd>" Close"
                            <span class="memory-transfer-spacer"></span>
                            <button class="memory-transfer-btn"
                                on:click=move |_| set_show_panel.set(Some("download"))
                                title="Export memories to JSON"
                            ><IconDownload size=13 />" Export"</button>
                            <button class="memory-transfer-btn"
                                on:click=move |_| set_show_panel.set(Some("upload"))
                                title="Import memories from JSON"
                            ><IconUpload size=13 />" Import"</button>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            // Download panel
            {move || {
                if show_panel.get() != Some("download") {
                    return view! { <div></div> }.into_any();
                }
                view! {
                    <div class="memory-transfer-panel">
                        <div class="memory-transfer-panel-header">
                            <span class="memory-transfer-panel-title">"Export Memories"</span>
                            <button class="memory-transfer-close"
                                on:click=move |_| set_show_panel.set(None)
                            ><IconX size=13 /></button>
                        </div>
                        <div class="memory-transfer-scopes">
                            <label class="memory-transfer-scope">
                                <input type="checkbox"
                                    prop:checked=move || dl_global.get()
                                    on:change=move |ev| set_dl_global.set(event_target_checked(&ev))
                                />" Global"
                            </label>
                            <label class="memory-transfer-scope">
                                <input type="checkbox"
                                    prop:checked=move || dl_project.get()
                                    on:change=move |ev| set_dl_project.set(event_target_checked(&ev))
                                />" Project"
                            </label>
                            <label class="memory-transfer-scope">
                                <input type="checkbox"
                                    prop:checked=move || dl_session.get()
                                    on:change=move |ev| set_dl_session.set(event_target_checked(&ev))
                                />" Session"
                            </label>
                        </div>
                        <button class="memory-create-btn memory-transfer-action"
                            on:click=handle_download
                            disabled=move || busy.get() || (!dl_global.get() && !dl_project.get() && !dl_session.get())
                        >
                            <IconDownload size=14 />
                            {move || if busy.get() { " Exporting..." } else { " Download JSON" }}
                        </button>
                    </div>
                }.into_any()
            }}

            // Upload panel
            {move || {
                if show_panel.get() != Some("upload") {
                    return view! { <div></div> }.into_any();
                }
                view! {
                    <div class="memory-transfer-panel">
                        <div class="memory-transfer-panel-header">
                            <span class="memory-transfer-panel-title">"Import Memories"</span>
                            <button class="memory-transfer-close"
                                on:click=move |_| { set_show_panel.set(None); set_ul_status.set(String::new()); }
                            ><IconX size=13 /></button>
                        </div>
                        <label class="memory-transfer-file-label">
                            <input type="file" accept=".json"
                                class="memory-transfer-file-input"
                                on:change=handle_file.clone()
                                disabled=move || busy.get()
                            />
                            <span class="memory-create-btn memory-transfer-action">
                                <IconUpload size=14 />
                                {move || if busy.get() { " Importing..." } else { " Choose JSON file" }}
                            </span>
                        </label>
                        {move || {
                            let s = ul_status.get();
                            if s.is_empty() {
                                view! { <span></span> }.into_any()
                            } else {
                                view! { <span class="memory-transfer-status">{s}</span> }.into_any()
                            }
                        }}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

/// Trigger a browser file download from a string.
fn trigger_download(content: &str, filename: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from_str(content));
    let mut opts = web_sys::BlobPropertyBag::new();
    opts.type_("application/json");
    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&arr, &opts) {
        Ok(b) => b,
        Err(_) => return,
    };
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };
    let a = match document.create_element("a") {
        Ok(el) => el,
        Err(_) => return,
    };
    let _ = a.set_attribute("href", &url);
    let _ = a.set_attribute("download", filename);
    let _ = a.set_attribute("style", "display:none");
    let body = match document.body() {
        Some(b) => b,
        None => return,
    };
    let _ = body.append_child(&a);
    if let Some(html_a) = a.dyn_ref::<web_sys::HtmlElement>() {
        html_a.click();
    }
    let _ = body.remove_child(&a);
    let _ = web_sys::Url::revoke_object_url(&url);
}

fn event_target_checked(ev: &web_sys::Event) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        .map(|i| i.checked())
        .unwrap_or(false)
}
