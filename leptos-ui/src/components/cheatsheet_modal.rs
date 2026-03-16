//! CheatsheetModal — keyboard shortcuts reference.
//! Matches React `CheatsheetModal.tsx`.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use leptos::prelude::*;

struct ShortcutEntry {
    key: &'static str,
    description: &'static str,
}

struct ShortcutSection {
    title: &'static str,
    entries: Vec<ShortcutEntry>,
}

fn build_sections() -> Vec<ShortcutSection> {
    vec![
        ShortcutSection {
            title: "General",
            entries: vec![
                ShortcutEntry {
                    key: "Cmd+Shift+P",
                    description: "Command Palette",
                },
                ShortcutEntry {
                    key: "Cmd+Shift+N",
                    description: "New Session",
                },
                ShortcutEntry {
                    key: "Cmd+B",
                    description: "Toggle Sidebar",
                },
                ShortcutEntry {
                    key: "Cmd+`",
                    description: "Toggle Terminal",
                },
                ShortcutEntry {
                    key: "Cmd+Shift+E",
                    description: "Toggle Editor",
                },
                ShortcutEntry {
                    key: "Cmd+Shift+G",
                    description: "Toggle Git Panel",
                },
                ShortcutEntry {
                    key: "?",
                    description: "Toggle Cheatsheet",
                },
            ],
        },
        ShortcutSection {
            title: "Chat",
            entries: vec![
                ShortcutEntry {
                    key: "Enter",
                    description: "Send message",
                },
                ShortcutEntry {
                    key: "Shift+Enter",
                    description: "New line",
                },
                ShortcutEntry {
                    key: "/",
                    description: "Slash command",
                },
                ShortcutEntry {
                    key: "Cmd+'",
                    description: "Focus input",
                },
            ],
        },
        ShortcutSection {
            title: "Modals",
            entries: vec![
                ShortcutEntry {
                    key: "Cmd+Shift+T",
                    description: "Todo Panel",
                },
                ShortcutEntry {
                    key: "Cmd+Shift+S",
                    description: "Session Selector",
                },
                ShortcutEntry {
                    key: "Cmd+Shift+K",
                    description: "Context Input",
                },
                ShortcutEntry {
                    key: "Cmd+,",
                    description: "Settings",
                },
                ShortcutEntry {
                    key: "Esc",
                    description: "Close top modal",
                },
            ],
        },
        ShortcutSection {
            title: "Slash Commands",
            entries: vec![
                ShortcutEntry {
                    key: "/new",
                    description: "New session",
                },
                ShortcutEntry {
                    key: "/model",
                    description: "Switch model",
                },
                ShortcutEntry {
                    key: "/theme",
                    description: "Change theme",
                },
                ShortcutEntry {
                    key: "/compact",
                    description: "Compact context",
                },
                ShortcutEntry {
                    key: "/undo",
                    description: "Undo last edit",
                },
                ShortcutEntry {
                    key: "/redo",
                    description: "Redo last edit",
                },
                ShortcutEntry {
                    key: "/fork",
                    description: "Fork session",
                },
                ShortcutEntry {
                    key: "/share",
                    description: "Share session",
                },
                ShortcutEntry {
                    key: "/agent",
                    description: "Switch agent",
                },
                ShortcutEntry {
                    key: "/terminal",
                    description: "Run in terminal",
                },
            ],
        },
        ShortcutSection {
            title: "Navigation",
            entries: vec![
                ShortcutEntry {
                    key: "Up/Down",
                    description: "Navigate items",
                },
                ShortcutEntry {
                    key: "Tab",
                    description: "Switch tabs/sections",
                },
                ShortcutEntry {
                    key: "Enter",
                    description: "Select / Confirm",
                },
            ],
        },
    ]
}

/// Cheatsheet modal component.
#[component]
pub fn CheatsheetModal(on_close: Callback<()>) -> impl IntoView {
    let sections = build_sections();

    view! {
        <ModalOverlay on_close=on_close class="cheatsheet-modal">
            <div class="cheatsheet-header">
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect width="20" height="16" x="2" y="4" rx="2" ry="2"/>
                    <path d="M6 8h.001"/><path d="M10 8h.001"/><path d="M14 8h.001"/>
                    <path d="M18 8h.001"/><path d="M8 12h.001"/><path d="M12 12h.001"/>
                    <path d="M16 12h.001"/><path d="M7 16h10"/>
                </svg>
                <span>"Keybindings"</span>
                <button class="cheatsheet-close" on:click=move |_| on_close.run(())>
                    <IconX size=14 class="w-3.5 h-3.5" />
                </button>
            </div>
            <div class="cheatsheet-body">
                {sections.into_iter().map(|section| {
                    view! {
                        <div class="cheatsheet-section">
                            <div class="cheatsheet-section-title">{section.title}</div>
                            {section.entries.into_iter().map(|entry| {
                                view! {
                                    <div class="cheatsheet-row">
                                        <kbd class="cheatsheet-key">{entry.key}</kbd>
                                        <span class="cheatsheet-desc">{entry.description}</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }
                }).collect_view()}
            </div>
            <div class="cheatsheet-footer">
                <kbd>"Esc"</kbd>" Close"
                <kbd>"?"</kbd>" Toggle"
            </div>
        </ModalOverlay>
    }
}
