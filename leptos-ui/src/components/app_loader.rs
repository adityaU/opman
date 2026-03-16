//! App loader spinner component.

use leptos::prelude::*;

#[component]
pub fn AppLoader() -> impl IntoView {
    view! {
        <div class="flex items-center justify-center w-full h-full bg-bg">
            <div class="w-7 h-7 rounded-full animate-spin"
                style="border: 2.5px solid color-mix(in srgb, var(--color-primary) 20%, transparent); border-top-color: var(--color-primary);">
            </div>
        </div>
    }
}
