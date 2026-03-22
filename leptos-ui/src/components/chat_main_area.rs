//! ChatMainArea — center column with sidebar, messages, input, and side panels.
//! Matches React `ChatMainArea.tsx`.

use leptos::prelude::*;
use std::collections::HashSet;

use crate::components::icons::*;
use crate::components::chat_sidebar::ChatSidebar;
use crate::components::message_timeline::MessageTimeline;
use crate::components::permission_dock::PermissionDock;
use crate::components::prompt_input::PromptInput;
use crate::components::question_dock::QuestionDock;
use crate::components::search_bar::SearchBar;
use crate::components::terminal_panel::TerminalPanel;
use crate::components::code_editor_panel::CodeEditorPanel;
use crate::components::git_panel::GitPanel;
use crate::components::debug_overlay::DebugPanel;
use crate::hooks::use_assistant_state::AssistantState;
use crate::hooks::use_bookmarks::BookmarkState;
use crate::hooks::use_chat_handlers::ChatHandlers;
use crate::hooks::use_mobile_state::{MobilePanel, MobileState};
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_model_state::ModelState;
use crate::hooks::use_panel_state::{FocusedPanel, PanelState};
use crate::hooks::use_resizable::{use_resizable, ResizableOptions, ResizeDirection};
use crate::hooks::use_sse_state::{ConnectionStatus, SessionStatus, SseState};
use crate::components::debug_overlay::dbg_log;

/// ChatMainArea component.
#[component]
pub fn ChatMainArea(
    sse: SseState,
    panels: PanelState,
    mobile: MobileState,
    modal_state: ModalState,
) -> impl IntoView {
    let sidebar_open = panels.sidebar_open;
    let focused = panels.focused;
    let terminal_open = panels.terminal.open;
    let terminal_mounted = panels.terminal.mounted;
    let editor_open = panels.editor.open;
    let editor_mounted = panels.editor.mounted;
    let git_open = panels.git.open;
    let git_mounted = panels.git.mounted;
    let debug_open = panels.debug.open;
    let debug_mounted = panels.debug.mounted;
    let session_status = sse.session_status;
    let connection_status = sse.connection_status;
    let mobile_sidebar_open = mobile.sidebar_open;
    let active_panel = mobile.active_panel;

    // Hide the floating chat header when a non-chat panel sheet covers the screen (phone only).
    // On tablet the header stays visible since tablet uses desktop split panels, not sheets.
    let header_hidden = Memo::new(move |_| {
        let mode = mobile.device_mode.get();
        if !mode.uses_panel_sheets() {
            return false;
        }
        matches!(active_panel.get(), Some(p) if p != MobilePanel::Opencode)
    });

    // Get model/agent/memory from context (provided by ChatLayoutInner)
    let model_state = expect_context::<ModelState>();
    let assistant = expect_context::<AssistantState>();
    let handlers = expect_context::<ChatHandlers>();
    let bookmarks = expect_context::<BookmarkState>();

    // Build bookmark callbacks matching React's isBookmarked / onToggleBookmark pattern
    let is_bookmarked_cb = {
        let bm = bookmarks;
        Callback::new(move |message_id: String| -> bool {
            bm.is_bookmarked(&message_id)
        })
    };
    let on_toggle_bookmark_cb = {
        let bm = bookmarks;
        Callback::new(move |(message_id, session_id, role, preview): (String, String, String, String)| {
            bm.toggle_bookmark(&message_id, &session_id, &role, &preview);
        })
    };

    // Build send prompt callback for prompt cards (matches React onSendPrompt → handleSend)
    let on_send_prompt_cb = {
        let send = handlers.handle_send.clone();
        Callback::new(move |text: String| {
            send.run((text, None));
        })
    };

    // Active session ID as a memo for passing to MessageTimeline/MessageTurn.
    // Uses the guarded tracked session ID to avoid re-evaluation on every app_state change.
    let session_id_memo = Memo::new(move |_| sse.tracked_session_id_reactive());

    // Resizable sidebar — matches React usePanelState.ts
    let sidebar_resize = use_resizable(ResizableOptions {
        initial_size: 280.0,
        min_size: 200.0,
        max_size: 500.0,
        direction: ResizeDirection::Horizontal,
        reverse: false,
    });

    // Resizable terminal — matches React usePanelState.ts
    let terminal_resize = use_resizable(ResizableOptions {
        initial_size: 250.0,
        min_size: 120.0,
        max_size: 600.0,
        direction: ResizeDirection::Vertical,
        reverse: true,
    });

    // Resizable side panel (editor/git) — matches React usePanelState.ts
    let side_panel_resize = use_resizable(ResizableOptions {
        initial_size: 500.0,
        min_size: 300.0,
        max_size: 900.0,
        direction: ResizeDirection::Horizontal,
        reverse: true,
    });

    let has_side_panel = Memo::new(move |_| editor_open.get() || git_open.get() || debug_open.get());

    // One-way latch: becomes true the first time any side panel is mounted,
    // never reverts. This ensures the side-panel container DOM is created
    // exactly once and subsequent mounts of the other panel don't cause a
    // full recreate.
    let side_ever_mounted = {
        let (sig, set_sig) = signal(false);
        Effect::new(move |_| {
            let em = editor_mounted.get();
            let gm = git_mounted.get();
            let dm = debug_mounted.get();
            if !sig.get_untracked() && (em || gm || dm) {
                dbg_log(&format!("[CMA] side_ever_mounted latch -> true (editor={}, git={}, debug={})", em, gm, dm));
                set_sig.set(true);
            }
        });
        sig
    };

    // Use derived signals to avoid subscribing to the full monolithic app_state.
    let active_project = sse.derived_active_project;
    let active_session_id = Memo::new(move |_| sse.tracked_session_id_reactive());
    let project_name = Memo::new(move |_| {
        active_project
            .get()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "opman".to_string())
    });

    // Compute sub-session IDs: direct children of the active session.
    // Matches React ChatLayout.tsx subSessionIds useMemo.
    let sub_session_ids = Memo::new(move |_| {
        let asid = match active_session_id.get() {
            Some(s) => s,
            None => return HashSet::<String>::new(),
        };
        let proj = match active_project.get() {
            Some(p) => p,
            None => return HashSet::new(),
        };
        proj.sessions
            .iter()
            .filter(|s| s.parent_id == asid)
            .map(|s| s.id.clone())
            .collect::<HashSet<String>>()
    });

    // Filtered permissions: active-session + direct sub-session items only.
    // Matches React: allPermissions = [...permissions, ...crossSessionPermissions.filter(p => subSessionIds.has(p.sessionID))]
    // Since Leptos stores all items in one flat Vec (no active/cross split), we just filter:
    // include item if session_id == activeSessionId OR session_id is in subSessionIds.
    let all_permissions = Memo::new(move |_| {
        let asid = active_session_id.get();
        let subs = sub_session_ids.get();
        sse.permissions
            .get()
            .into_iter()
            .filter(|p| {
                match &asid {
                    Some(a) if &p.session_id == a => true,
                    _ => subs.contains(&p.session_id),
                }
            })
            .collect::<Vec<_>>()
    });

    let all_questions = Memo::new(move |_| {
        let asid = active_session_id.get();
        let subs = sub_session_ids.get();
        sse.questions
            .get()
            .into_iter()
            .filter(|q| {
                match &asid {
                    Some(a) if &q.session_id == a => true,
                    _ => subs.contains(&q.session_id),
                }
            })
            .collect::<Vec<_>>()
    });

    // Permission reply handler
    let perm_sse = sse;
    let on_permission_reply = Callback::new(move |(request_id, reply): (String, String)| {
        let perms_sig = perm_sse.set_permissions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match crate::api::reply_permission(&request_id, &reply).await {
                Ok(()) => {
                    // Remove from local state optimistically
                    perms_sig.update(|perms| {
                        perms.retain(|p| p.id != rid);
                    });
                }
                Err(e) => {
                    log::error!("Failed to reply permission: {}", e);
                }
            }
        });
    });

    // Question reply handler
    let q_sse = sse;
    let on_question_reply = Callback::new(move |(request_id, answers): (String, Vec<Vec<String>>)| {
        let q_sig = q_sse.set_questions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match crate::api::reply_question(&request_id, &answers).await {
                Ok(()) => {
                    q_sig.update(|qs| {
                        qs.retain(|q| q.id != rid);
                    });
                }
                Err(e) => {
                    log::error!("Failed to reply question: {}", e);
                }
            }
        });
    });

    // Question dismiss handler
    let d_sse = sse;
    let on_question_dismiss = Callback::new(move |request_id: String| {
        let q_sig = d_sse.set_questions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match crate::api::reject_question(&request_id).await {
                Ok(()) => {
                    q_sig.update(|qs| {
                        qs.retain(|q| q.id != rid);
                    });
                }
                Err(e) => {
                    log::error!("Failed to dismiss question: {}", e);
                }
            }
        });
    });

    // Navigate to session — matches React handleOpenSession callback.
    // Calls handleSelectSession(sid, activeProjectIdx).
    let on_go_to_session = {
        let select = handlers.handle_select_session.clone();
        let proj_idx = sse.derived_active_project_idx;
        Callback::new(move |session_id: String| {
            let project_idx = proj_idx.get_untracked();
            select.run((session_id, project_idx));
        })
    };

    let sidebar_size = sidebar_resize.size;
    let terminal_size = terminal_resize.size;
    let side_size = side_panel_resize.size;

    view! {
        <div             class="chat-content">
            // Sidebar
            {move || {
                if sidebar_open.get() {
                    Some(view! {
                        <div
                            style:width=move || format!("{}px", sidebar_size.get())
                            style:flex-shrink="0"
                            class="h-full"
                            class:panel-dimmed=move || focused.get() != FocusedPanel::Sidebar
                            on:mousedown=move |_| panels.focus_sidebar()
                        >
                            <ChatSidebar
                                sse=sse
                                modal_state=modal_state
                                mobile_open=mobile_sidebar_open
                                on_close=Callback::new(move |_| mobile.close_sidebar())
                            />
                        </div>
                        // Sidebar resize handle
                        <div
                            class=move || {
                                let base = "resize-handle resize-handle-horizontal w-1 cursor-col-resize bg-transparent hover:bg-border-active transition-colors flex-shrink-0";
                                if sidebar_resize.is_dragging.get() {
                                    format!("{} dragging bg-primary/40", base)
                                } else {
                                    base.to_string()
                                }
                            }
                            style="touch-action: none"
                            on:mousedown=move |e| sidebar_resize.start_drag(e)
                            on:touchstart=move |e| sidebar_resize.start_drag_touch(e)
                        />
                    })
                } else {
                    None
                }
            }}

            // Main chat area
            <div
                class="chat-main"
                class:panel-dimmed=move || focused.get() != FocusedPanel::Chat
                on:mousedown=move |_| panels.focus_chat()
            >
                // Mobile floating status pill (React: div.chat-mobile-header)
                <div
                    class="chat-mobile-header"
                    class:header-hidden=move || header_hidden.get()
                >
                    <button
                        class="mobile-status-pill"
                        on:click=move |_| mobile.toggle_sidebar()
                        aria-label=move || {
                            if mobile.sidebar_open.get() { "Close sidebar" } else { "Open sessions" }
                        }
                    >
                        // Sparkles icon (Lucide)
                        <span class="mobile-pill-icon"><IconSparkles size=14 /></span>
                        <span class="mobile-project-name">{project_name}</span>
                        {move || {
                            if session_status.get() == SessionStatus::Busy {
                                Some(view! { <span class="mobile-pill-busy" /> })
                            } else {
                                None
                            }
                        }}
                        {move || {
                            let status = connection_status.get();
                            let cls = format!(
                                "mobile-pill-connection mobile-pill-connection-{}",
                                status.as_str()
                            );
                            if status == ConnectionStatus::Connected {
                                view! {
                                    <span class=cls>
                                        <IconWifi size=12 />
                                    </span>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <span class=cls>
                                        <IconWifiOff size=12 />
                                    </span>
                                }
                                .into_any()
                            }
                        }}
                    </button>
                    // Command button (React: button.mobile-cmd-btn with Command icon)
                    <button
                        class="mobile-cmd-btn"
                        on:click=move |_| modal_state.toggle(crate::hooks::use_modal_state::ModalName::CommandPalette)
                        aria-label="Open command palette"
                    >
                        // Command icon (Lucide ⌘)
                        <IconCommand size=14 />
                    </button>
                </div>

                // In-session search bar (React: conditionally rendered when searchBarOpen)
                {move || {
                    if modal_state.is_open_tracked(ModalName::SearchBar) {
                        let messages = sse.messages.get();
                        let ms = modal_state;
                        Some(view! {
                            <SearchBar
                                messages=messages
                                on_close=Callback::new(move |_: ()| {
                                    ms.close(ModalName::SearchBar);
                                })
                                on_matches_changed=Callback::new(move |(match_ids, active_id): (HashSet<String>, Option<String>)| {
                                    ms.set_search_match_ids.set(match_ids.into_iter().collect());
                                    ms.set_active_search_match_id.set(active_id);
                                })
                            />
                        }.into_any())
                    } else {
                        None
                    }
                }}

                // Message timeline
                <MessageTimeline
                    sse=sse
                    on_scroll_direction=Callback::new(move |direction: String| {
                        mobile.handle_scroll_direction(&direction);
                    })
                    on_send_prompt=on_send_prompt_cb
                    is_bookmarked=is_bookmarked_cb
                    on_toggle_bookmark=on_toggle_bookmark_cb
                    session_id=session_id_memo
                />

                // Permission dock
                <PermissionDock
                    permissions=all_permissions
                    active_session_id=active_session_id
                    on_reply=on_permission_reply
                    on_go_to_session=on_go_to_session
                />

                // Question dock
                <QuestionDock
                    questions=all_questions
                    active_session_id=active_session_id
                    on_reply=on_question_reply
                    on_dismiss=on_question_dismiss
                    on_go_to_session=on_go_to_session
                />

                // Prompt input (React: div.mobile-input-wrapper > PromptInput)
                <div class=move || {
                    if mobile.input_hidden.get() {
                        "mobile-input-wrapper mobile-input-hidden"
                    } else {
                        "mobile-input-wrapper"
                    }
                }>
                    <PromptInput
                        sse=sse
                        on_send=handlers.handle_send
                        on_command={
                            let hc = handlers.handle_command.clone();
                            Callback::new(move |(cmd, args): (String, String)| {
                                hc.run((cmd, Some(args)));
                            })
                        }
                        on_abort=handlers.handle_abort
                        on_open_model_picker=Callback::new(move |_: ()| {
                            modal_state.open(ModalName::ModelPicker);
                        })
                        on_open_agent_picker=Callback::new(move |_: ()| {
                            modal_state.open(ModalName::AgentPicker);
                        })
                        on_open_memory=Callback::new(move |_: ()| {
                            modal_state.open(ModalName::Memory);
                        })
                        on_content_change=Callback::new(move |has_content: bool| {
                            // React: if user has content, don't allow scroll-hide
                            mobile.has_prompt_content.set(has_content);
                        })
                        current_model=Signal::derive(move || {
                            model_state.current_model.get().unwrap_or_default()
                        })
                        current_agent=Signal::derive(move || {
                            model_state.current_agent.get()
                        })
                        active_memory_labels=Signal::derive(move || {
                            assistant.active_memory_items.get()
                                .iter()
                                .map(|item| item.label.clone())
                                .collect::<Vec<_>>()
                        })
                    />
                </div>

            // Terminal panel area — mount-once pattern: once mounted, stays in DOM
            // but hidden via display:none when closed (preserves terminal state).
            // IMPORTANT: only track `terminal_mounted` here (one-way latch).
            // All open/close visibility uses style:display bindings so toggling
            // the terminal never re-creates the DOM subtree.
            {move || {
                let m = terminal_mounted.get();
                dbg_log(&format!("[CMA] terminal mount closure ran, terminal_mounted={}", m));
                if m {
                    Some(view! {
                            // Terminal resize handle
                            <div
                                class=move || {
                                    let base = "resize-handle resize-handle-vertical h-1 cursor-row-resize bg-transparent hover:bg-border-active transition-colors flex-shrink-0 border-t border-border-subtle";
                                    if terminal_resize.is_dragging.get() {
                                        format!("{} dragging bg-primary/40", base)
                                    } else {
                                        base.to_string()
                                    }
                                }
                                style="touch-action: none"
                                style:display=move || if terminal_open.get() { "" } else { "none" }
                                on:mousedown=move |e| terminal_resize.start_drag(e)
                                on:touchstart=move |e| terminal_resize.start_drag_touch(e)
                            />
                            <div
                                style:height=move || format!("{}px", terminal_size.get())
                                style:flex-shrink="0"
                                style:display=move || if terminal_open.get() { "" } else { "none" }
                            >
                                <TerminalPanel
                                    session_id=Signal::derive(move || active_session_id.get().map(|s| s.to_string()))
                                    on_close=Callback::new(move |_| panels.terminal.close())
                                    visible=Signal::derive(move || terminal_open.get())
                                    mcp_agent_active=Signal::derive(move || session_status.get() == SessionStatus::Busy)
                                />
                            </div>
                        })
                    } else {
                        None
                    }
                }}
            </div>

            // Side panel: Editor or Git
            // IMPORTANT: the outer closure tracks only the one-way latch
            // `side_ever_mounted` so the DOM subtree is created exactly once
            // and never destroyed. All open/close visibility is via style:display.
            {move || {
                let sem = side_ever_mounted.get();
                dbg_log(&format!("[CMA] side panel mount closure ran, side_ever_mounted={}", sem));
                if sem {
                    Some(view! {
                        // Side panel resize handle
                        <div
                            class=move || {
                                let base = "resize-handle resize-handle-horizontal w-1 cursor-col-resize bg-transparent hover:bg-border-active transition-colors flex-shrink-0";
                                if side_panel_resize.is_dragging.get() {
                                    format!("{} dragging bg-primary/40", base)
                                } else {
                                    base.to_string()
                                }
                            }
                            style="touch-action: none"
                            style:display=move || if has_side_panel.get() { "" } else { "none" }
                            on:mousedown=move |e| side_panel_resize.start_drag(e)
                            on:touchstart=move |e| side_panel_resize.start_drag_touch(e)
                        />
                        <div
                            class="side-panel flex flex-col"
                            class:panel-dimmed=move || focused.get() != FocusedPanel::Side
                            style:width=move || format!("{}px", side_size.get())
                            style:flex-shrink="0"
                            style:display=move || if has_side_panel.get() { "" } else { "none" }
                            on:mousedown=move |_| panels.focus_side()
                        >
                            // Editor section
                            {move || {
                                let em = editor_mounted.get();
                                dbg_log(&format!("[CMA] editor mount closure ran, editor_mounted={}", em));
                                if em {
                                    Some(view! {
                                        <div
                                            class="side-panel-section flex flex-col flex-1"
                                            style:display=move || if editor_open.get() { "" } else { "none" }
                                        >
                                            <div class="side-panel-header flex items-center gap-1.5 px-3 py-1.5 bg-bg-panel border-b border-border-subtle">
                                                // FileCode icon (lucide)
                                                <IconFileCode size=14 />
                                                <span class="text-text text-xs font-medium">"Editor"</span>
                                                // MCP agent indicator (React checks web_editor prefix keys;
                                                // for now we approximate with session busy status)
                                                {move || {
                                                    if session_status.get() == SessionStatus::Busy {
                                                        Some(view! {
                                                            <span class="mcp-agent-indicator" title="AI agent active">
                                                                <span class="mcp-agent-dot" />
                                                            </span>
                                                        })
                                                    } else {
                                                        None
                                                    }
                                                }}
                                                <span class="flex-1" />
                                                <button
                                                    class="side-panel-close text-text-muted hover:text-text"
                                                    on:click=move |_| panels.editor.close()
                                                    aria-label="Close editor panel"
                                                >
                                                    <IconX size=14 />
                                                </button>
                                            </div>
                                            <div class="side-panel-body flex-1 overflow-hidden">
                                                <CodeEditorPanel panels=panels />
                                            </div>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }}
                            // Git section
                            {move || {
                                let gm = git_mounted.get();
                                dbg_log(&format!("[CMA] git mount closure ran, git_mounted={}", gm));
                                if gm {
                                    Some(view! {
                                        <div
                                            class="side-panel-section flex flex-col flex-1"
                                            style:display=move || if git_open.get() { "" } else { "none" }
                                        >
                                            <div class="side-panel-header flex items-center gap-1.5 px-3 py-1.5 bg-bg-panel border-b border-border-subtle">
                                                // GitBranch icon (lucide)
                                                <IconGitBranch size=14 />
                                                <span class="text-text text-xs font-medium">"Git"</span>
                                                <span class="flex-1" />
                                                <button
                                                    class="side-panel-close text-text-muted hover:text-text"
                                                    on:click=move |_| panels.git.close()
                                                    aria-label="Close git panel"
                                                >
                                                    <IconX size=14 />
                                                </button>
                                            </div>
                                            <div class="side-panel-body flex-1 overflow-hidden">
                                                <GitPanel
                                                    panels=panels
                                                    on_send_to_ai=Callback::new(move |text: String| {
                                                        handlers.handle_send.run((text, None));
                                                    })
                                                />
                                            </div>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }}
                            // Debug section
                            {move || {
                                if !debug_mounted.get() {
                                    return None;
                                }
                                Some(view! {
                                    <div
                                        class="side-panel-section flex flex-col flex-1"
                                        style:display=move || if debug_open.get() { "" } else { "none" }
                                    >
                                        <div class="side-panel-header flex items-center gap-1.5 px-3 py-1.5 bg-bg-panel border-b border-border-subtle">
                                            <IconActivity size=14 />
                                            <span class="text-text text-xs font-medium">"Debug"</span>
                                            <span class="flex-1" />
                                            <button
                                                class="side-panel-close text-text-muted hover:text-text"
                                                on:click=move |_| panels.debug.close()
                                                aria-label="Close debug panel"
                                            >
                                                <IconX size=14 />
                                            </button>
                                        </div>
                                        <div class="side-panel-body flex-1 overflow-hidden">
                                            <DebugPanel />
                                        </div>
                                    </div>
                                })
                            }}
                        </div>
                    })
                } else {
                    None
                }
            }}
        </div>
    }
}
