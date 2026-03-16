//! PanelFloatingHeader — pill-style floating header for phone panel sheets.
//! Shows panel name + command palette shortcut.
//! On phone it floats over fullscreen panels.
//! Tablet + desktop use their own panel headers.

use leptos::prelude::*;

use crate::components::icons::*;
use crate::hooks::use_mobile_state::MobilePanel;
use crate::hooks::use_modal_state::{ModalName, ModalState};

/// Floating panel header — pill-style with panel name + command icon.
#[component]
pub fn PanelFloatingHeader(panel: MobilePanel, modal_state: ModalState) -> impl IntoView {
    let icon = match panel {
        MobilePanel::Git => view! { <IconGitBranch size=14 /> }.into_any(),
        MobilePanel::Editor => view! { <IconFileCode size=14 /> }.into_any(),
        MobilePanel::Terminal => view! { <IconTerminal size=14 /> }.into_any(),
        MobilePanel::Opencode => view! { <IconMessageCircle size=14 /> }.into_any(),
    };
    let name = panel.display_name();

    view! {
        <div class="panel-floating-header">
            <div class="panel-floating-header-pill">
                {icon}
                <span class="panel-floating-header-name">{name}</span>
            </div>
            <button
                class="panel-floating-header-cmd"
                on:click=move |_| modal_state.open(ModalName::CommandPalette)
                aria-label="Open command palette"
            >
                <IconCommand size=14 />
            </button>
        </div>
    }
}
