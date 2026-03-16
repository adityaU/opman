//! CommandPalette — command launcher.
//! Matches React `command-palette/CommandPalette.tsx` exactly.

use crate::components::icons::*;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;
use leptos::prelude::*;

// ── Types ───────────────────────────────────────────────────────────

#[derive(Clone)]
struct PaletteItem {
    id: &'static str,
    category: &'static str,
    label: &'static str,
    description: &'static str,
    shortcut: &'static str,
    handler: Callback<()>,
}

impl PartialEq for PaletteItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

struct PaletteGroup {
    category: &'static str,
    items: Vec<PaletteItem>,
}

// ── Helpers ─────────────────────────────────────────────────────────

fn filter_items(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    if query.is_empty() {
        return items.to_vec();
    }
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|item| {
            item.label.to_lowercase().contains(&q) || item.description.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

fn group_items(items: &[PaletteItem]) -> Vec<PaletteGroup> {
    let mut groups: Vec<PaletteGroup> = Vec::new();
    for item in items {
        if let Some(group) = groups.iter_mut().find(|g| g.category == item.category) {
            group.items.push(item.clone());
        } else {
            groups.push(PaletteGroup {
                category: item.category,
                items: vec![item.clone()],
            });
        }
    }
    groups
}

// ── Component ───────────────────────────────────────────────────────

/// Command palette component — exact React parity.
#[component]
pub fn CommandPalette(
    on_close: Callback<()>,
    on_command: Callback<(String, Option<String>)>,
    on_new_session: Callback<()>,
    modal_state: ModalState,
    panels: PanelState,
    session_id: Option<String>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (selected_index, set_selected_index) = signal(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let palette_ref = NodeRef::<leptos::html::Div>::new();

    // Focus input on mount
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Build items — matches React items.ts exactly
    let close_and_open = move |name: ModalName| {
        Callback::new(move |_: ()| {
            on_close.run(());
            modal_state.open(name);
        })
    };

    // model-picker does NOT call onClose first (React behavior)
    let open_only = move |name: ModalName| {
        Callback::new(move |_: ()| {
            modal_state.open(name);
        })
    };

    let on_close_cb = on_close;
    let on_new_session_cb = on_new_session;

    let items: Vec<PaletteItem> = {
        let mut v = vec![
            // ── Sessions ────────────────────────────────────
            PaletteItem {
                id: "new-session",
                category: "Sessions",
                label: "New Session",
                description: "",
                shortcut: "\u{2318}\u{21e7}N",
                handler: Callback::new(move |_| {
                    on_close_cb.run(());
                    on_new_session_cb.run(());
                }),
            },
            // ── Core ────────────────────────────────────────
            PaletteItem {
                id: "model-picker",
                category: "Core",
                label: "Choose Model",
                description: "",
                shortcut: "\u{2318}'",
                handler: open_only(ModalName::ModelPicker),
            },
            // ── Layout ──────────────────────────────────────
            PaletteItem {
                id: "toggle-sidebar",
                category: "Layout",
                label: "Toggle Sidebar",
                description: "",
                shortcut: "\u{2318}B",
                handler: {
                    let panels = panels;
                    Callback::new(move |_: ()| {
                        on_close.run(());
                        panels.toggle_sidebar();
                    })
                },
            },
            PaletteItem {
                id: "toggle-terminal",
                category: "Layout",
                label: "Toggle Terminal",
                description: "",
                shortcut: "\u{2318}`",
                handler: {
                    let panels = panels;
                    Callback::new(move |_: ()| {
                        on_close.run(());
                        panels.terminal.toggle();
                    })
                },
            },
            // ── Core (continued) ────────────────────────────
            PaletteItem {
                id: "cheatsheet",
                category: "Core",
                label: "Keyboard Shortcuts",
                description: "",
                shortcut: "?",
                handler: close_and_open(ModalName::Cheatsheet),
            },
            // ── Sessions (continued) ────────────────────────
            PaletteItem {
                id: "session-selector",
                category: "Sessions",
                label: "Select Session",
                description: "Search across all projects",
                shortcut: "\u{2318}\u{21e7}S",
                handler: close_and_open(ModalName::SessionSelector),
            },
            // ── Core (continued) ────────────────────────────
            PaletteItem {
                id: "settings",
                category: "Core",
                label: "Settings",
                description: "Configure panels and theme",
                shortcut: "\u{2318},",
                handler: close_and_open(ModalName::Settings),
            },
            // ── Sessions (continued) ────────────────────────
            PaletteItem {
                id: "watcher",
                category: "Sessions",
                label: "Session Watcher",
                description: "Monitor and auto-continue sessions",
                shortcut: "\u{2318}\u{21e7}W",
                handler: close_and_open(ModalName::Watcher),
            },
            // ── Analysis ────────────────────────────────────
            PaletteItem {
                id: "context-window",
                category: "Analysis",
                label: "Context Window",
                description: "View token usage breakdown",
                shortcut: "\u{2318}\u{21e7}C",
                handler: close_and_open(ModalName::ContextWindow),
            },
            PaletteItem {
                id: "diff-review",
                category: "Analysis",
                label: "Diff Review",
                description: "Review file changes made by AI",
                shortcut: "\u{2318}\u{21e7}D",
                handler: close_and_open(ModalName::DiffReview),
            },
            // ── Search ──────────────────────────────────────
            PaletteItem {
                id: "search",
                category: "Search",
                label: "Search in Conversation",
                description: "Find text in the current session",
                shortcut: "\u{2318}F",
                handler: close_and_open(ModalName::SearchBar),
            },
            PaletteItem {
                id: "cross-search",
                category: "Search",
                label: "Search All Sessions",
                description: "Search across all sessions in project",
                shortcut: "\u{2318}\u{21e7}F",
                handler: close_and_open(ModalName::CrossSearch),
            },
            // ── Layout (continued) ──────────────────────────
            PaletteItem {
                id: "split-view",
                category: "Layout",
                label: "Split View",
                description: "View two sessions side by side",
                shortcut: "\u{2318}\\",
                handler: close_and_open(ModalName::SplitView),
            },
            // ── Sessions (continued) ────────────────────────
            PaletteItem {
                id: "session-graph",
                category: "Sessions",
                label: "Session Graph",
                description: "View session dependency tree",
                shortcut: "\u{2318}\u{21e7}H",
                handler: close_and_open(ModalName::SessionGraph),
            },
            PaletteItem {
                id: "session-dashboard",
                category: "Sessions",
                label: "Session Dashboard",
                description: "Overview of all running sessions",
                shortcut: "\u{2318}\u{21e7}O",
                handler: close_and_open(ModalName::SessionDashboard),
            },
            PaletteItem {
                id: "activity-feed",
                category: "Sessions",
                label: "Activity Feed",
                description: "View real-time session activity",
                shortcut: "\u{2318}\u{21e7}A",
                handler: close_and_open(ModalName::ActivityFeed),
            },
            // ── Assistant ───────────────────────────────────
            PaletteItem {
                id: "notification-prefs",
                category: "Assistant",
                label: "Notification Preferences",
                description: "Configure session alerts",
                shortcut: "\u{2318}\u{21e7}I",
                handler: close_and_open(ModalName::NotificationPrefs),
            },
            PaletteItem {
                id: "assistant-center",
                category: "Assistant",
                label: "Assistant Center",
                description: "Open the assistant cockpit",
                shortcut: "\u{2318}\u{21e7}.",
                handler: close_and_open(ModalName::AssistantCenter),
            },
            PaletteItem {
                id: "assistant-inbox",
                category: "Assistant",
                label: "Assistant Inbox",
                description: "Review everything that needs attention",
                shortcut: "\u{2318}\u{21e7}U",
                handler: close_and_open(ModalName::Inbox),
            },
            PaletteItem {
                id: "missions",
                category: "Assistant",
                label: "Missions",
                description: "Track high-level goals above sessions",
                shortcut: "\u{2318}\u{21e7}M",
                handler: close_and_open(ModalName::Missions),
            },
            PaletteItem {
                id: "delegation-board",
                category: "Assistant",
                label: "Delegation Board",
                description: "Track delegated work and linked outputs",
                shortcut: "\u{2318}\u{21e7}B",
                handler: close_and_open(ModalName::Delegation),
            },
            PaletteItem {
                id: "routines",
                category: "Assistant",
                label: "Routines",
                description: "Manage recurring assistant workflows",
                shortcut: "\u{2318}\u{21e7}R",
                handler: close_and_open(ModalName::Routines),
            },
            PaletteItem {
                id: "autonomy",
                category: "Assistant",
                label: "Autonomy",
                description: "Choose proactive assistant mode",
                shortcut: "\u{2318}\u{21e7}J",
                handler: close_and_open(ModalName::Autonomy),
            },
            PaletteItem {
                id: "personal-memory",
                category: "Assistant",
                label: "Personal Memory",
                description: "Store preferences and working norms",
                shortcut: "\u{2318}\u{21e7}Y",
                handler: close_and_open(ModalName::Memory),
            },
            PaletteItem {
                id: "workspace-manager",
                category: "Assistant",
                label: "Workspaces",
                description: "Save and restore workspace layouts",
                shortcut: "\u{2318}\u{21e7}L",
                handler: close_and_open(ModalName::WorkspaceManager),
            },
            // ── System ──────────────────────────────────────
            PaletteItem {
                id: "system-monitor",
                category: "System",
                label: "System Monitor",
                description: "htop-like system resource monitor",
                shortcut: "",
                handler: close_and_open(ModalName::SystemMonitor),
            },
            PaletteItem {
                id: "refresh",
                category: "System",
                label: "Refresh Page",
                description: "Reload the application",
                shortcut: "",
                handler: Callback::new(move |_: ()| {
                    on_close.run(());
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().reload();
                    }
                }),
            },
        ];

        // Session-specific items (only when session active)
        if session_id.is_some() {
            let on_close2 = on_close;
            let on_cmd = on_command;
            let cmd = move |name: &'static str| {
                Callback::new(move |_: ()| {
                    on_close2.run(());
                    on_cmd.run((name.to_string(), None));
                })
            };
            v.push(PaletteItem {
                id: "todo-panel",
                category: "Sessions",
                label: "Todo Panel",
                description: "View session todos",
                shortcut: "\u{2318}\u{21e7}T",
                handler: close_and_open(ModalName::TodoPanel),
            });
            v.push(PaletteItem {
                id: "context-input",
                category: "Sessions",
                label: "Send Context",
                description: "Send context to the AI session",
                shortcut: "\u{2318}\u{21e7}K",
                handler: close_and_open(ModalName::ContextInput),
            });
            v.push(PaletteItem {
                id: "compact",
                category: "Sessions",
                label: "Compact History",
                description: "Compact conversation to reduce tokens",
                shortcut: "",
                handler: cmd("compact"),
            });
            v.push(PaletteItem {
                id: "undo",
                category: "Sessions",
                label: "Undo",
                description: "Undo last action",
                shortcut: "",
                handler: cmd("undo"),
            });
            v.push(PaletteItem {
                id: "redo",
                category: "Sessions",
                label: "Redo",
                description: "Redo last action",
                shortcut: "",
                handler: cmd("redo"),
            });
            v.push(PaletteItem {
                id: "fork",
                category: "Sessions",
                label: "Fork Session",
                description: "Create a copy of this session",
                shortcut: "",
                handler: cmd("fork"),
            });
            v.push(PaletteItem {
                id: "share",
                category: "Sessions",
                label: "Share Session",
                description: "Get a shareable link",
                shortcut: "",
                handler: cmd("share"),
            });
        }

        v
    };

    // Filtered items memo
    let items_clone = items.clone();
    let filtered = Memo::new(move |_| filter_items(&items_clone, &query.get()));

    // Reset selected on query change
    Effect::new(move |_| {
        let _ = query.get();
        set_selected_index.set(0);
    });

    // Keyboard handler
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected_index.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected_index.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                let idx = selected_index.get_untracked();
                if let Some(item) = items.get(idx) {
                    item.handler.run(());
                }
            }
            "Escape" => {
                e.prevent_default();
                on_close.run(());
            }
            _ => {}
        }
    };

    // Click outside handler
    let on_backdrop_click = move |_: web_sys::MouseEvent| {
        on_close.run(());
    };

    view! {
        <div class="modal-backdrop" on:click=on_backdrop_click>
            <div
                class="command-palette"
                node_ref=palette_ref
                role="dialog"
                aria-modal="true"
                on:click=|e: web_sys::MouseEvent| e.stop_propagation()
            >
                <div class="command-palette-input-row">
                    <IconSearch size=16 class="command-palette-icon" />
                    <input
                        class="command-palette-input"
                        node_ref=input_ref
                        type="text"
                        placeholder="Type a command..."
                        prop:value=move || query.get()
                        on:input=move |e| set_query.set(event_target_value(&e))
                        on:keydown=on_keydown
                    />
                </div>
                <div class="command-palette-results">
                    {move || {
                        let items = filtered.get();
                        if items.is_empty() {
                            view! { <div class="command-palette-empty">"No commands found"</div> }.into_any()
                        } else {
                            let groups = group_items(&items);
                            let selected = selected_index.get();
                            let mut global_idx = 0usize;
                            view! {
                                <div>
                                    {groups.into_iter().map(|group| {
                                        let section_items: Vec<_> = group.items.into_iter().map(|item| {
                                            let idx = global_idx;
                                            global_idx += 1;
                                            let is_selected = idx == selected;
                                            let handler = item.handler;
                                            view! {
                                                <button
                                                    class=if is_selected { "command-palette-item selected" } else { "command-palette-item" }
                                                    on:click=move |_| handler.run(())
                                                    on:mouseenter=move |_| set_selected_index.set(idx)
                                                >
                                                    <div class="command-palette-item-left">
                                                        <span class="command-palette-label">{item.label}</span>
                                                        {if !item.description.is_empty() {
                                                            Some(view! { <span class="command-palette-desc">{item.description}</span> })
                                                        } else {
                                                            None
                                                        }}
                                                    </div>
                                                    {if !item.shortcut.is_empty() {
                                                        Some(view! { <kbd class="command-palette-shortcut">{item.shortcut}</kbd> })
                                                    } else {
                                                        None
                                                    }}
                                                </button>
                                            }
                                        }).collect_view();

                                        view! {
                                            <div class="command-palette-section">
                                                <div class="command-palette-section-title">{group.category}</div>
                                                {section_items}
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
