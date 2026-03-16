//! Toast notification system — signals-based, matches React `useToast` + `ToastContainer`.

use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;

use crate::components::icons::*;
use crate::components::message_turn::{parse_markdown_segments, ContentSegment};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A single toast notification.
#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub toast_type: ToastType,
}

/// Toast severity type.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

impl ToastType {
    /// React class suffix: `toast-success`, `toast-error`, etc.
    pub fn css_suffix(&self) -> &'static str {
        match self {
            ToastType::Success => "success",
            ToastType::Error => "error",
            ToastType::Info => "info",
            ToastType::Warning => "warning",
        }
    }
}

/// Toast context — provide at app root, use from anywhere.
/// Matches React `useToast` — `add()` returns toast ID, `remove()` cancels timer.
///
/// Uses `Arc<Mutex<HashMap>>` for timer handles so the type is `Send + Sync`
/// (required by Leptos `provide_context`).
#[derive(Clone, Copy)]
pub struct ToastContext {
    pub toasts: ReadSignal<Vec<Toast>>,
    set_toasts: WriteSignal<Vec<Toast>>,
    /// Timer handles keyed by toast ID for cancellation on manual dismiss.
    /// Stored in a StoredValue to keep ToastContext Copy.
    timers: StoredValue<Arc<Mutex<HashMap<u64, i32>>>>,
}

impl ToastContext {
    /// Add a toast and return its ID. Matches React `addToast(message, type, durationMs)`.
    pub fn add(&self, message: impl Into<String>, toast_type: ToastType, duration_ms: u32) -> u64 {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let toast = Toast {
            id,
            message: message.into(),
            toast_type,
        };
        self.set_toasts.update(|list| list.push(toast));

        if duration_ms > 0 {
            // Use window.setTimeout so we get a handle we can cancel
            if let Some(window) = web_sys::window() {
                let set_toasts = self.set_toasts;
                let timers_sv = self.timers;
                let cb = wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
                    set_toasts.update(|list| list.retain(|t| t.id != id));
                    if let Some(timers) = timers_sv.try_get_value() {
                        if let Ok(mut map) = timers.lock() {
                            map.remove(&id);
                        }
                    }
                });
                if let Ok(handle) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    duration_ms as i32,
                ) {
                    if let Some(timers) = self.timers.try_get_value() {
                        if let Ok(mut map) = timers.lock() {
                            map.insert(id, handle);
                        }
                    }
                }
                cb.forget();
            }
        }

        id
    }

    /// Remove a toast by ID and cancel its auto-dismiss timer.
    /// Matches React `removeToast(id)`.
    pub fn remove(&self, id: u64) {
        self.set_toasts.update(|list| list.retain(|t| t.id != id));
        if let Some(timers) = self.timers.try_get_value() {
            if let Ok(mut map) = timers.lock() {
                if let Some(handle) = map.remove(&id) {
                    if let Some(window) = web_sys::window() {
                        window.clear_timeout_with_handle(handle);
                    }
                }
            }
        }
    }

    pub fn success(&self, msg: impl Into<String>) -> u64 {
        self.add(msg, ToastType::Success, 3000)
    }

    pub fn error(&self, msg: impl Into<String>) -> u64 {
        self.add(msg, ToastType::Error, 5000)
    }

    pub fn info(&self, msg: impl Into<String>) -> u64 {
        self.add(msg, ToastType::Info, 3000)
    }

    pub fn warning(&self, msg: impl Into<String>) -> u64 {
        self.add(msg, ToastType::Warning, 4000)
    }
}

/// Provide toast context at app root.
pub fn provide_toast_context() -> ToastContext {
    let (toasts, set_toasts) = signal(Vec::<Toast>::new());
    let ctx = ToastContext {
        toasts,
        set_toasts,
        timers: StoredValue::new(Arc::new(Mutex::new(HashMap::new()))),
    };
    provide_context(ctx);
    ctx
}

/// Use toast from any child component.
pub fn use_toast() -> ToastContext {
    expect_context::<ToastContext>()
}

/// Toast container component — matches React `ToastContainer` markup/classes exactly.
/// Structure:
///   .toast-container
///     .toast.toast-${type}
///       .toast-icon
///       .toast-message.toast-markdown  (renders markdown)
///       .toast-close
#[component]
pub fn ToastContainer() -> impl IntoView {
    let ctx = use_toast();

    view! {
        <div class="toast-container">
            <For
                each=move || ctx.toasts.get()
                key=|toast| toast.id
                let:toast
            >
                <ToastItem toast=toast />
            </For>
        </div>
    }
}

/// A single toast item — matches React markup.
#[component]
fn ToastItem(toast: Toast) -> impl IntoView {
    let ctx = use_toast();
    let id = toast.id;
    let toast_type = toast.toast_type;
    let type_suffix = toast_type.css_suffix();
    let class_str = format!("toast toast-{type_suffix}");

    let dismiss = move |_: web_sys::MouseEvent| {
        ctx.remove(id);
    };

    // Render markdown content for the toast message
    let rendered_md = render_toast_markdown(&toast.message);

    view! {
        <div class=class_str>
            <span class="toast-icon">
                {match toast_type {
                    ToastType::Success => view! { <IconCheckCircle2 size=14 /> }.into_any(),
                    ToastType::Error => view! { <IconXCircle size=14 /> }.into_any(),
                    ToastType::Info => view! {
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="12" cy="12" r="10"/>
                            <line x1="12" y1="16" x2="12" y2="12"/>
                            <line x1="12" y1="8" x2="12.01" y2="8"/>
                        </svg>
                    }.into_any(),
                    ToastType::Warning => view! {
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                            <line x1="12" y1="9" x2="12" y2="13"/>
                            <line x1="12" y1="17" x2="12.01" y2="17"/>
                        </svg>
                    }.into_any(),
                }}
            </span>
            <div class="toast-message toast-markdown">
                {rendered_md}
            </div>
            <button
                class="toast-close"
                on:click=dismiss
                aria-label="Dismiss notification"
            >
                <IconX size=12 />
            </button>
        </div>
    }
}

/// Render markdown for toast messages using the shared markdown parser.
/// Produces HTML fragments matching React's ReactMarkdown output inside `.toast-markdown`.
fn render_toast_markdown(text: &str) -> impl IntoView {
    let segments = parse_markdown_segments(text);
    let views: Vec<_> = segments
        .into_iter()
        .map(|seg| match seg {
            ContentSegment::Html(html) => view! { <div inner_html=html /> }.into_any(),
            ContentSegment::FencedCode { language, code } => {
                let lang_class = if language.is_empty() {
                    String::new()
                } else {
                    format!("language-{language}")
                };
                view! {
                    <pre>
                        <code class=lang_class>{code}</code>
                    </pre>
                }
                .into_any()
            }
        })
        .collect();
    views
}
