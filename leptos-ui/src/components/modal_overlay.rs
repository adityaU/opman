//! ModalOverlay — shared base wrapper for all modal dialogs.
//! Provides backdrop, click-outside-to-close, Escape-to-close, focus trap,
//! and consistent structure.

use crate::hooks::use_focus_trap::use_focus_trap;
use crate::hooks::use_keyboard::use_escape;
use leptos::prelude::*;

/// Shared modal overlay wrapper.
/// Renders a backdrop + inner dialog container with focus trapping.
/// Pressing Escape closes the modal (handled globally via `use_escape`).
#[component]
pub fn ModalOverlay(
    /// Called when user clicks backdrop or presses Escape
    on_close: Callback<()>,
    /// Extra CSS class(es) for the inner dialog div
    #[prop(optional)]
    class: &'static str,
    /// Children rendered inside the dialog
    children: Children,
) -> impl IntoView {
    let modal_ref = NodeRef::<leptos::html::Div>::new();
    use_focus_trap(modal_ref);
    use_escape(on_close);

    view! {
        <div
            class="modal-backdrop"
            on:click=move |_| on_close.run(())
        >
            <div
                class=format!("{}", class)
                node_ref=modal_ref
                role="dialog"
                aria-modal="true"
                on:click=move |e| e.stop_propagation()
            >
                {children()}
            </div>
        </div>
    }
}
