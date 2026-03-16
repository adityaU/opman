//! SettingsModal — application settings hub.
//! Matches React `settings-modal/SettingsModal.tsx`.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;
use leptos::prelude::*;

// ── Types ───────────────────────────────────────────────────────────

#[derive(Clone)]
enum SettingType {
    Action,
    Toggle,
}

#[derive(Clone)]
struct SettingItem {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    setting_type: SettingType,
    value: Option<bool>,
    handler: Callback<()>,
}

// ── Component ───────────────────────────────────────────────────────

/// Settings modal component.
#[component]
pub fn SettingsModal(
    on_close: Callback<()>,
    modal_state: ModalState,
    panels: PanelState,
) -> impl IntoView {
    let (selected_idx, set_selected_idx) = signal(0usize);

    let nav = move |to: ModalName| {
        Callback::new(move |_: ()| {
            on_close.run(());
            modal_state.open(to);
        })
    };

    let items: Vec<SettingItem> = vec![
        SettingItem {
            id: "theme",
            label: "Theme",
            description: "Change color theme & mode",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::ThemeSelector),
        },
        SettingItem {
            id: "keybindings",
            label: "Keybindings",
            description: "View keyboard shortcuts",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Cheatsheet),
        },
        SettingItem {
            id: "notifications",
            label: "Notifications",
            description: "Browser notification settings",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::NotificationPrefs),
        },
        SettingItem {
            id: "assistant",
            label: "Assistant Center",
            description: "AI assistant dashboard",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::AssistantCenter),
        },
        SettingItem {
            id: "inbox",
            label: "Inbox",
            description: "Notifications & signals",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Inbox),
        },
        SettingItem {
            id: "missions",
            label: "Missions",
            description: "Mission tracker",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Missions),
        },
        SettingItem {
            id: "memory",
            label: "Personal Memory",
            description: "Memory items management",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Memory),
        },
        SettingItem {
            id: "autonomy",
            label: "Autonomy",
            description: "Autonomy mode settings",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Autonomy),
        },
        SettingItem {
            id: "routines",
            label: "Routines",
            description: "Automated routines",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Routines),
        },
        SettingItem {
            id: "delegation",
            label: "Delegation Board",
            description: "Delegated work items",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::Delegation),
        },
        SettingItem {
            id: "workspaces",
            label: "Workspaces",
            description: "Save & restore workspaces",
            setting_type: SettingType::Action,
            value: None,
            handler: nav(ModalName::WorkspaceManager),
        },
        SettingItem {
            id: "sidebar",
            label: "Sidebar",
            description: "Toggle sidebar panel",
            setting_type: SettingType::Toggle,
            value: Some(panels.sidebar_open.get_untracked()),
            handler: Callback::new(move |_| panels.toggle_sidebar()),
        },
        SettingItem {
            id: "terminal",
            label: "Terminal",
            description: "Toggle terminal panel",
            setting_type: SettingType::Toggle,
            value: Some(panels.terminal.open.get_untracked()),
            handler: Callback::new(move |_| panels.terminal.toggle()),
        },
        SettingItem {
            id: "editor",
            label: "Editor",
            description: "Toggle editor panel",
            setting_type: SettingType::Toggle,
            value: Some(panels.editor.open.get_untracked()),
            handler: Callback::new(move |_| panels.editor.toggle()),
        },
        SettingItem {
            id: "git",
            label: "Git Panel",
            description: "Toggle git panel",
            setting_type: SettingType::Toggle,
            value: Some(panels.git.open.get_untracked()),
            handler: Callback::new(move |_| panels.git.toggle()),
        },
    ];

    let items_len = items.len();

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                set_selected_idx.update(|i| *i = (*i + 1).min(items_len - 1));
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected_idx.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" | " " => {
                e.prevent_default();
                // Will be handled via rendered items
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="settings-modal">
            <div class="settings-header">
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
                    <circle cx="12" cy="12" r="3"/>
                </svg>
                <span>"Settings"</span>
                <button class="settings-close" on:click=move |_| on_close.run(())>
                    <IconX size=14 class="w-3.5 h-3.5" />
                </button>
            </div>
            <div class="settings-list" on:keydown=on_keydown tabindex=0>
                {items.into_iter().enumerate().map(|(idx, item)| {
                    let handler = item.handler;
                    let is_toggle = matches!(item.setting_type, SettingType::Toggle);

                    view! {
                        <button
                            class=move || if selected_idx.get() == idx { "settings-item selected" } else { "settings-item" }
                            on:click=move |_| handler.run(())
                            on:mouseenter=move |_| set_selected_idx.set(idx)
                        >
                            <div class="settings-item-left">
                                <div class="settings-item-text">
                                    <span class="settings-item-label">{item.label}</span>
                                    <span class="settings-item-desc">{item.description}</span>
                                </div>
                            </div>
                            {if is_toggle {
                                let on = item.value.unwrap_or(false);
                                Some(view! {
                                    <span class=if on { "settings-toggle on" } else { "settings-toggle off" }>
                                        {if on { "ON" } else { "OFF" }}
                                    </span>
                                })
                            } else {
                                Some(view! { <span class="settings-action-arrow">"›"</span> })
                            }}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="settings-footer">
                <kbd>"Up/Down"</kbd>" Navigate "
                <kbd>"Enter"</kbd>" Toggle / Open "
                <kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
