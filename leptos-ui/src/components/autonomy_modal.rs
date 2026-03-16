//! AutonomyModal — simple 4-option selector for autonomy behavior profile.
//! Matches React `AutonomyModal.tsx`.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use leptos::prelude::*;

struct ModeEntry {
    id: &'static str,
    label: &'static str,
    description: &'static str,
}

const MODES: &[ModeEntry] = &[
    ModeEntry {
        id: "observe",
        label: "Observe",
        description: "Only react when directly asked.",
    },
    ModeEntry {
        id: "nudge",
        label: "Nudge",
        description: "Surface gentle reminders and summaries.",
    },
    ModeEntry {
        id: "continue",
        label: "Continue",
        description: "Allow limited proactive continuation flows.",
    },
    ModeEntry {
        id: "autonomous",
        label: "Autonomous",
        description: "Allow the highest available proactive behavior.",
    },
];

#[component]
pub fn AutonomyModal(
    on_close: Callback<()>,
    /// Current autonomy mode string: "observe" | "nudge" | "continue" | "autonomous"
    mode: String,
    /// Called with the new mode string when user clicks a mode button.
    on_change: Callback<String>,
) -> impl IntoView {
    view! {
        <ModalOverlay on_close=on_close class="autonomy-modal">
            <div class="autonomy-header">
                <div class="autonomy-header-left">
                    <svg class="icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 8V4H8" />
                        <rect width="16" height="12" x="4" y="8" rx="2" />
                        <path d="M2 14h2" />
                        <path d="M20 14h2" />
                        <path d="M15 13v2" />
                        <path d="M9 13v2" />
                    </svg>
                    <h3>"Autonomy"</h3>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close autonomy settings">
                    <IconX size=16 />
                </button>
            </div>
            <div class="autonomy-body">
                <div class="assistant-center-briefing">
                    <div class="assistant-center-briefing-title">"Behavior Profile"</div>
                    <div class="assistant-center-briefing-summary">
                        "Choose how proactive opman should feel across reminders, summaries, and autonomous continuation."
                    </div>
                </div>
                {MODES.iter().map(|entry| {
                    let active = mode == entry.id;
                    let id = entry.id.to_string();
                    let on_change = on_change;
                    view! {
                        <button
                            class=if active { "autonomy-item active" } else { "autonomy-item" }
                            on:click=move |_| on_change.run(id.clone())
                        >
                            <div class="autonomy-item-label">{entry.label}</div>
                            <div class="autonomy-item-desc">{entry.description}</div>
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </ModalOverlay>
    }
}
