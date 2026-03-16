//! MobileDock — bottom dock navigation for phone and tablet.
//! Phone uses dock + fullscreen panel sheets; tablet uses dock + desktop split panels.
//! Desktop uses side-panel / terminal-tray layout only (dock hidden via CSS).

use leptos::prelude::*;

use crate::components::code_editor_panel::CodeEditorPanel;
use crate::components::git_panel::GitPanel;
use crate::components::icons::*;
use crate::components::panel_floating_header::PanelFloatingHeader;
use crate::components::terminal_panel::TerminalPanel;
use crate::hooks::use_mobile_state::{MobilePanel, MobileState};
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::{SessionStatus, SseState};

/// MobileDock component.
#[component]
pub fn MobileDock(
    mobile: MobileState,
    modal_state: ModalState,
    panels: PanelState,
    sse: SseState,
) -> impl IntoView {
    let active_panel = mobile.active_panel;
    let input_hidden = mobile.input_hidden;
    let dock_collapsed = mobile.dock_collapsed;
    let device_mode = mobile.device_mode;

    // On tablet, dock panel buttons toggle desktop side panels instead of mobile sheets.
    // On phone, they toggle the mobile active_panel which drives fullscreen sheets.
    let handle_panel_tap = move |panel: MobilePanel| {
        let mode = device_mode.get_untracked();
        if mode.uses_panel_sheets() {
            // Phone: toggle mobile panel sheet
            mobile.toggle_panel(panel);
        } else {
            // Tablet: toggle desktop side panel / terminal tray
            match panel {
                MobilePanel::Opencode => {
                    // "Chat" tap on tablet: no-op or collapse dock
                    mobile.collapse_dock();
                }
                MobilePanel::Git => panels.git.toggle(),
                MobilePanel::Editor => panels.editor.toggle(),
                MobilePanel::Terminal => panels.terminal.toggle(),
            }
        }
    };

    // Compose visibility: "visible" | "consumed" | "" (hidden)
    let compose_class = Memo::new(move |_| {
        if !input_hidden.get() {
            return "";
        }
        // Hide compose FAB when a non-chat panel is active (phone only)
        if device_mode.get().is_phone() {
            let panel = active_panel.get();
            if panel.is_some() && panel != Some(MobilePanel::Opencode) {
                return "";
            }
        }
        if dock_collapsed.get() {
            "visible"
        } else {
            "consumed"
        }
    });

    // Track which panels have been mounted (once mounted, keep alive for state persistence).
    let (git_mounted, set_git_mounted) = signal(false);
    let (editor_mounted, set_editor_mounted) = signal(false);
    let (terminal_mounted, set_terminal_mounted) = signal(false);

    // When a panel becomes active, mark it as mounted (one-way latch).
    // Guard: only set if not already true to avoid re-notifying dependents.
    Effect::new(move |_| match active_panel.get() {
        Some(MobilePanel::Git) if !git_mounted.get_untracked() => set_git_mounted.set(true),
        Some(MobilePanel::Editor) if !editor_mounted.get_untracked() => {
            set_editor_mounted.set(true)
        }
        Some(MobilePanel::Terminal) if !terminal_mounted.get_untracked() => {
            set_terminal_mounted.set(true)
        }
        _ => {}
    });

    // Derived signals for terminal panel props
    let active_session_id = Memo::new(move |_| sse.tracked_session_id_reactive());
    let session_status = sse.session_status;

    // Dock compose inside-dock visibility (reactive)
    let dock_compose_visible = Memo::new(move |_| {
        let hidden = input_hidden.get();
        let collapsed = dock_collapsed.get();
        let panel = active_panel.get();
        let mode = device_mode.get();

        if !hidden || collapsed {
            return false;
        }
        // Hide compose in dock for non-chat panels (phone only)
        if mode.is_phone() {
            return panel.is_none() || panel == Some(MobilePanel::Opencode);
        }
        true
    });

    // Panel sheet CSS class — fullscreen overlay (phone + tablet)
    let panel_sheet_class = move |panel: MobilePanel| {
        let is_active = active_panel.get() == Some(panel);
        let mut cls = String::from("mobile-panel-sheet");
        if is_active {
            cls.push_str(" mobile-panel-active");
        }
        cls
    };

    view! {
        // Visible on phone + tablet (hidden on desktop via CSS)
        <div class="dock-container">
            // Compose button — visibility driven by CSS classes
            <button
                class=move || format!("mobile-compose-btn {}", compose_class.get())
                on:click=move |_| mobile.handle_compose_button_tap()
                aria-label="Compose message"
            >
                <IconPenSquare size=20 />
            </button>

            // Collapsed FAB
            <button
                class=move || {
                    let visible = dock_collapsed.get() && input_hidden.get();
                    format!(
                        "mobile-dock-fab {}",
                        if visible { "visible" } else { "" }
                    )
                }
                on:click=move |_| mobile.expand_dock()
                aria-label="Open navigation"
            >
                <IconMenu size=22 />
            </button>

            // Expanded dock
            <nav
                class=move || {
                    let collapsed = dock_collapsed.get();
                    format!(
                        "mobile-dock {}",
                        if collapsed { "dock-collapsed" } else { "" }
                    )
                }
                aria-label="Navigation"
            >
                <div class="mobile-dock-inner">
                    // Compose button inside dock
                    <button
                        class=move || {
                            format!(
                                "mobile-dock-btn dock-compose-btn {}",
                                if dock_compose_visible.get() { "dock-compose-visible" } else { "" }
                            )
                        }
                        on:click=move |_| mobile.handle_compose_button_tap()
                        aria-label="Compose message"
                    >
                        <IconPenSquare size=20 />
                    </button>

                    // Chat
                    <button
                        class=move || {
                            let is_active = active_panel.get().map(|p| p == MobilePanel::Opencode).unwrap_or(active_panel.get().is_none());
                            format!(
                                "mobile-dock-btn {}",
                                if is_active { "active" } else { "" }
                            )
                        }
                        on:click=move |_| handle_panel_tap(MobilePanel::Opencode)
                        aria-label="Chat"
                    >
                        <IconMessageCircle size=18 />
                        <span class="dock-label">"Chat"</span>
                    </button>

                    // Git
                    <button
                        class=move || {
                            let mode = device_mode.get();
                            let is_active = if mode.uses_panel_sheets() {
                                active_panel.get() == Some(MobilePanel::Git)
                            } else {
                                panels.git.open.get()
                            };
                            format!(
                                "mobile-dock-btn {}",
                                if is_active { "active" } else { "" }
                            )
                        }
                        on:click=move |_| handle_panel_tap(MobilePanel::Git)
                        aria-label="Git"
                    >
                        <IconGitBranch size=18 />
                        <span class="dock-label">"Git"</span>
                    </button>

                    // Editor
                    <button
                        class=move || {
                            let mode = device_mode.get();
                            let is_active = if mode.uses_panel_sheets() {
                                active_panel.get() == Some(MobilePanel::Editor)
                            } else {
                                panels.editor.open.get()
                            };
                            format!(
                                "mobile-dock-btn {}",
                                if is_active { "active" } else { "" }
                            )
                        }
                        on:click=move |_| handle_panel_tap(MobilePanel::Editor)
                        aria-label="Editor"
                    >
                        <IconFileCode size=18 />
                        <span class="dock-label">"Editor"</span>
                    </button>

                    // Terminal
                    <button
                        class=move || {
                            let mode = device_mode.get();
                            let is_active = if mode.uses_panel_sheets() {
                                active_panel.get() == Some(MobilePanel::Terminal)
                            } else {
                                panels.terminal.open.get()
                            };
                            format!(
                                "mobile-dock-btn {}",
                                if is_active { "active" } else { "" }
                            )
                        }
                        on:click=move |_| handle_panel_tap(MobilePanel::Terminal)
                        aria-label="Terminal"
                    >
                        <IconTerminal size=18 />
                        <span class="dock-label">"Term"</span>
                    </button>

                    // AI / Assistant
                    <button
                        class=move || {
                            let is_active = modal_state.is_open_tracked(ModalName::AssistantCenter);
                            format!(
                                "mobile-dock-btn {}",
                                if is_active { "active" } else { "" }
                            )
                        }
                        on:click=move |_| modal_state.open(ModalName::AssistantCenter)
                        aria-label="Assistant"
                    >
                        <IconSparkles size=18 />
                        <span class="dock-label">"AI"</span>
                    </button>
                </div>
            </nav>

            // ── Panel sheets (phone only — tablet uses desktop split panels) ──

            // Git panel sheet
            {move || {
                if !device_mode.get().uses_panel_sheets() {
                    return None;
                }
                if git_mounted.get() {
                    Some(view! {
                        <div class=move || panel_sheet_class(MobilePanel::Git)>
                            <PanelFloatingHeader
                                panel=MobilePanel::Git
                                modal_state=modal_state
                            />
                            <div class="mobile-panel-content">
                                <GitPanel panels=panels />
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}

            // Editor panel sheet
            {move || {
                if !device_mode.get().uses_panel_sheets() {
                    return None;
                }
                if editor_mounted.get() {
                    Some(view! {
                        <div class=move || panel_sheet_class(MobilePanel::Editor)>
                            <PanelFloatingHeader
                                panel=MobilePanel::Editor
                                modal_state=modal_state
                            />
                            <div class="mobile-panel-content">
                                <CodeEditorPanel panels=panels />
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}

            // Terminal panel sheet
            {move || {
                if !device_mode.get().uses_panel_sheets() {
                    return None;
                }
                if terminal_mounted.get() {
                    Some(view! {
                        <div class=move || panel_sheet_class(MobilePanel::Terminal)>
                            <PanelFloatingHeader
                                panel=MobilePanel::Terminal
                                modal_state=modal_state
                            />
                            <div class="mobile-panel-content">
                                <TerminalPanel
                                    session_id=Signal::derive(move || active_session_id.get().map(|s| s.to_string()))
                                    on_close=Callback::new(move |_| mobile.toggle_panel(MobilePanel::Terminal))
                                    visible=Signal::derive(move || active_panel.get() == Some(MobilePanel::Terminal))
                                    mcp_agent_active=Signal::derive(move || session_status.get() == SessionStatus::Busy)
                                />
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}
        </div>
    }
}
