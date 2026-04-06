//! AutoOpenModal — configure which tool-call accordions auto-open by default.
//! Uses localStorage persistence via `AutoOpenState` context.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::hooks::use_auto_open::{AutoOpenState, ToolCategory};
use leptos::prelude::*;

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn AutoOpenModal(on_close: Callback<()>) -> impl IntoView {
    let state = use_context::<AutoOpenState>().expect("AutoOpenState must be provided via context");

    view! {
        <ModalOverlay on_close=on_close class="notification-prefs-modal">
            <div class="notification-prefs-header">
                <h3>"Auto Open"</h3>
                <button on:click=move |_| on_close.run(()) aria-label="Close">
                    <IconX size=14 />
                </button>
            </div>

            <div class="notification-prefs-body">
                <div class="notification-prefs-permission">
                    <div class="notification-prefs-permission-info">
                        <span class="notification-prefs-permission-label">
                            "Choose which tool-call accordions expand automatically."
                        </span>
                    </div>
                </div>

                {ToolCategory::ALL.iter().map(|&cat| {
                    let config = state.config;

                    let toggle = move |_: web_sys::MouseEvent| {
                        state.toggle(cat);
                    };

                    let is_on = move || config.get().get_category(cat);

                    view! {
                        <div class="notification-prefs-item" on:click=toggle>
                            <div class="notification-prefs-item-left">
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none"
                                     stroke="currentColor" stroke-width="2"
                                     stroke-linecap="round" stroke-linejoin="round">
                                    <path d=cat.icon_path() />
                                </svg>
                                <div>
                                    <div class="notification-prefs-item-label">{cat.label()}</div>
                                    <div class="notification-prefs-item-desc">{cat.description()}</div>
                                </div>
                            </div>
                            <span class=move || {
                                if is_on() { "notification-prefs-badge on" } else { "notification-prefs-badge off" }
                            }>
                                {move || if is_on() { "ON" } else { "OFF" }}
                            </span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div class="notification-prefs-footer">
                "All toggles are OFF by default. Changes are saved immediately."
            </div>
        </ModalOverlay>
    }
}
