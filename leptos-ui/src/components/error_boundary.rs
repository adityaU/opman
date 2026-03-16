//! Error boundary component — catches panics and renders a fallback UI.
//! In WASM/Leptos CSR, there are no React-style error boundaries.
//! Instead we use a signal-based approach: wrap children and provide
//! a global error signal that components can set.

use leptos::prelude::*;

/// Global error state for the application.
#[derive(Clone)]
pub struct AppError {
    pub message: String,
}

/// Provide an error context and render children, or the error fallback.
#[component]
pub fn ErrorBoundary(children: Children) -> impl IntoView {
    let (error, set_error) = signal(Option::<AppError>::None);

    // Provide the error setter so any child can trigger it
    provide_context(set_error);

    let handle_reload = move |_: web_sys::MouseEvent| {
        if let Some(w) = web_sys::window() {
            let _ = w.location().reload();
        }
    };

    let handle_dismiss = move |_: web_sys::MouseEvent| {
        set_error.set(None);
    };

    // Render children eagerly
    let children_view = children();

    view! {
        // Error overlay — shown when error is set, hides children via CSS
        <div style:display=move || if error.get().is_some() { "flex" } else { "none" }
             class="items-center justify-center w-full h-screen bg-bg p-4 fixed inset-0 z-[100]">
            <div class="max-w-[480px] w-full p-6 rounded-lg bg-bg-panel border border-border text-center"
                style="box-shadow: 0 8px 32px rgba(0,0,0,0.15);">
                <h2 class="m-0 mb-3 text-lg font-semibold text-error">"Something went wrong"</h2>
                <p class="m-0 mb-4 text-sm text-text-muted leading-relaxed">
                    "An unexpected error occurred. You can try dismissing the error or reloading the page."
                </p>
                <pre class="m-0 mb-4 p-3 rounded-sm text-xs text-text-muted font-mono text-left whitespace-pre-wrap break-words overflow-x-auto max-h-[160px] overflow-y-auto"
                    style="background: rgba(255,255,255,0.04); border: 1px solid var(--color-border);">
                    {move || error.get().map(|e| e.message).unwrap_or_default()}
                </pre>
                <div class="flex gap-3 justify-center">
                    <button
                        class="px-5 py-2 border-none rounded-sm text-sm font-medium cursor-pointer transition-all hover:brightness-110 text-text"
                        style="background: rgba(255,255,255,0.08);"
                        on:click=handle_dismiss
                    >"Dismiss"</button>
                    <button
                        class="px-5 py-2 border-none rounded-sm text-sm font-medium cursor-pointer transition-all hover:brightness-110 bg-primary text-white"
                        on:click=handle_reload
                    >"Reload Page"</button>
                </div>
            </div>
        </div>

        // Children always rendered, hidden when error is shown
        {children_view}
    }
}
