//! ChatLayout — main orchestration hub.
//! Matches React `ChatLayout.tsx` — wires SSE state, panels, modals, mobile, keyboard,
//! assistant state, chat handlers, providers, model state, and all reactive hooks.

use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::hooks::use_back_navigation::use_back_navigation;
use crate::hooks::use_assistant_state::use_assistant_state;
use crate::hooks::use_bookmarks::use_bookmarks;
use crate::hooks::use_chat_callbacks::{use_chat_callbacks, ChatCallbackDeps};
use crate::hooks::use_chat_handlers::{use_chat_handlers, ChatHandlerDeps};
use crate::hooks::use_keyboard::{use_keyboard, KeyBinding};
use crate::hooks::use_mobile_state::{use_mobile_state, MobileState};
use crate::hooks::use_modal_state::{use_modal_state, ModalName, ModalState};
use crate::hooks::use_model_state::use_model_state;
use crate::hooks::use_notification_signals::use_notification_signals;
use crate::hooks::use_panel_state::{use_panel_state, PanelConfig, PanelState};
use crate::hooks::use_providers::use_providers;
use crate::hooks::use_pulse_actions::use_pulse_actions;
use crate::hooks::use_sse_state::{use_sse_state, ConnectionStatus, SseState};
use crate::hooks::use_url_restore::use_url_restore;
use crate::hooks::use_virtual_keyboard::use_virtual_keyboard;

use crate::components::chat_main_area::ChatMainArea;
use crate::components::mobile_dock::MobileDock;
use crate::components::modal_layer::ModalLayer;
use crate::components::status_bar::StatusBar;
use crate::components::theme_selector_modal::{get_persisted_theme_mode, ThemeMode};
use crate::components::toast::{ToastContext, ToastType};
use crate::components::debug_overlay::dbg_log;

/// ChatLayout component — the main authenticated view.
#[component]
pub fn ChatLayout() -> impl IntoView {
    // ── Core SSE state ──
    let sse = use_sse_state();

    // ── Panels ──
    let panels = use_panel_state(PanelConfig::default());

    // ── Modals ──
    let modal_state = use_modal_state();

    // ── Mobile ──
    let mobile = use_mobile_state();

    // ── Theme mode ──
    let (theme_mode, set_theme_mode) = signal(get_persisted_theme_mode());

    // Load initial app state
    {
        let set_app_state = sse.set_app_state;
        let set_connection = sse.set_connection_status;
        let set_loading = sse.set_is_loading_messages;
        let set_permissions = sse.set_permissions;
        let set_questions = sse.set_questions;
        let set_busy = sse.set_busy_sessions;
        let set_error = sse.set_error_sessions;
        let set_input = sse.set_input_sessions;
        let set_unseen = sse.set_unseen_sessions;
        leptos::task::spawn_local(async move {
            match crate::api::api_fetch::<crate::types::api::AppState>("/state").await {
                Ok(state) => {
                    // Bootstrap indicator sets from initial state
                    let mut busy = std::collections::HashSet::new();
                    let mut error = std::collections::HashSet::new();
                    let mut input = std::collections::HashSet::new();
                    let mut unseen = std::collections::HashSet::new();
                    for p in &state.projects {
                        busy.extend(p.busy_sessions.iter().cloned());
                        error.extend(p.error_sessions.iter().cloned());
                        input.extend(p.input_sessions.iter().cloned());
                        unseen.extend(p.unseen_sessions.iter().cloned());
                    }
                    set_busy.set(busy);
                    set_error.set(error);
                    set_input.set(input);
                    set_unseen.set(unseen);
                    set_app_state.set(Some(state));
                    set_connection.set(ConnectionStatus::Connected);
                }
                Err(e) => {
                    log::error!("Failed to load app state: {}", e);
                    set_connection.set(ConnectionStatus::Disconnected);
                }
            }
            set_loading.set(false);
        });

        // Load theme colors
        leptos::task::spawn_local(async move {
            match crate::api::fetch_theme().await {
                Ok(Some(colors)) => crate::theme::apply_theme_to_css(&colors),
                Ok(None) => {} // No custom theme, defaults are fine
                Err(e) => log::warn!("Failed to fetch theme: {}", e),
            }
        });

        // Load pending permissions and questions
        leptos::task::spawn_local(async move {
            match crate::api::fetch_pending().await {
                Ok(pending) => {
                    // Use the same manual parsing as the live SSE handler
                    // to handle field name mismatches (permission vs toolName, etc.)
                    use crate::sse::connection::session_handlers::{
                        parse_permission_from_props, parse_question_from_props,
                    };
                    let perms: Vec<crate::types::core::PermissionRequest> = pending
                        .permissions
                        .iter()
                        .filter_map(|v| parse_permission_from_props(v))
                        .collect();
                    let qs: Vec<crate::types::core::QuestionRequest> = pending
                        .questions
                        .iter()
                        .filter_map(|v| parse_question_from_props(v))
                        .collect();
                    if !perms.is_empty() {
                        set_permissions.set(perms);
                    }
                    if !qs.is_empty() {
                        set_questions.set(qs);
                    }
                }
                Err(e) => log::warn!("Failed to fetch pending: {}", e),
            }
        });
    }

    // Wire real SSE connections
    let toast_for_sse = leptos::prelude::use_context::<crate::components::toast::ToastContext>();
    crate::sse::connection::wire_sse(sse, toast_for_sse);

    // Track active session changes — implements the session-switch guard.
    // Uses derived memos that extract ONLY the active session ID,
    // so we only re-run when the session actually changes (not on title/time updates).
    {
        let sse_inner = sse;

        // Derive (project_index, active_session_id) via narrow derived signals.
        let active_session_key = Memo::new(move |_| {
            let proj = sse_inner.derived_active_project.get();
            proj.map(|p| {
                (sse_inner.derived_active_project_idx.get(), p.active_session.clone())
            })
        });

        Effect::new(move |_| {
            // Subscribe to the memo — only fires when project/session actually changes.
            let _key = active_session_key.get();
            sse_inner.track_active_session();
        });
    }

    // Per-project panel state: save current panel layout when switching projects,
    // restore the saved layout for the new project.
    {
        let project_idx = sse.derived_active_project_idx;
        let panels_inner = panels;
        Effect::new(move |prev_idx: Option<usize>| {
            let new_idx = project_idx.get();
            if let Some(old) = prev_idx {
                if old != new_idx {
                    panels_inner.save_for_project(old);
                    panels_inner.restore_for_project(new_idx);
                }
            }
            new_idx
        });
    }

    // Provide state as context for child components
    provide_context(sse);
    provide_context(panels);
    provide_context(crate::hooks::use_project_context::ProjectContext {
        index: sse.derived_active_project_idx,
    });
    provide_context(modal_state);
    provide_context(mobile);

    // ── Keyboard shortcuts ──
    {
        let panels = panels;
        let modal_state = modal_state;
        let mobile = mobile;

        use_keyboard(vec![
            // Toggle sidebar: Cmd+B
            KeyBinding::new("b", Callback::new(move |_| {
                panels.toggle_sidebar();
            }))
            .meta()
            .desc("Toggle Sidebar"),
            // Toggle terminal: Cmd+`
            KeyBinding::new("`", Callback::new(move |_| {
                panels.terminal.toggle();
            }))
            .meta()
            .desc("Toggle Terminal"),
            // Toggle editor: Cmd+Shift+E
            KeyBinding::new("e", Callback::new(move |_| {
                panels.editor.toggle();
            }))
            .meta()
            .shift()
            .desc("Toggle Editor"),
            // Toggle git: Cmd+Shift+G
            KeyBinding::new("g", Callback::new(move |_| {
                panels.git.toggle();
            }))
            .meta()
            .shift()
            .desc("Toggle Git"),
            // Toggle debug: Cmd+Shift+/
            KeyBinding::new("/", Callback::new(move |_| {
                panels.debug.toggle();
            }))
            .meta()
            .shift()
            .desc("Toggle Debug"),
            // Toggle health: Cmd+Shift+H
            KeyBinding::new("h", Callback::new(move |_| {
                modal_state.toggle(ModalName::ProcessHealth);
            }))
            .meta()
            .shift()
            .desc("Toggle Process Health"),
            // Command palette: Cmd+Shift+P
            KeyBinding::new("p", Callback::new(move |_| {
                modal_state.toggle(ModalName::CommandPalette);
            }))
            .meta()
            .shift()
            .desc("Command Palette"),
            // Command palette: Cmd+K (alternative)
            KeyBinding::new("k", Callback::new(move |_| {
                modal_state.toggle(ModalName::CommandPalette);
            }))
            .meta()
            .desc("Command Palette"),
            // Model picker: Cmd+M
            KeyBinding::new("m", Callback::new(move |_| {
                modal_state.toggle(ModalName::ModelPicker);
            }))
            .meta()
            .desc("Model Picker"),
            // Cheatsheet: Cmd+/
            KeyBinding::new("/", Callback::new(move |_| {
                modal_state.toggle(ModalName::Cheatsheet);
            }))
            .meta()
            .desc("Keyboard Shortcuts"),
            // Search: Cmd+F
            KeyBinding::new("f", Callback::new(move |_| {
                modal_state.toggle(ModalName::SearchBar);
            }))
            .meta()
            .desc("Search"),
            // Escape: close top modal
            KeyBinding::new("Escape", Callback::new(move |_| {
                if !modal_state.close_top_modal() {
                    // If no modal was open, close mobile sidebar
                    mobile.close_sidebar();
                }
            }))
            .desc("Close/Escape"),
        ]);
    }

    // One-way latch: once app state is loaded, stay true.
    // This prevents ChatLayoutInner from being re-created on every app_state update.
    // Uses derived_projects to avoid subscribing to the entire app_state.
    let initialized = RwSignal::new(false);
    Effect::new(move |_| {
        if !sse.derived_projects.get().is_empty() && !initialized.get_untracked() {
            dbg_log("[LAYOUT] ChatLayout initialized latch -> true");
            initialized.set(true);
        }
    });

    view! {
        <div class="chat-layout">
            {move || {
                let init = initialized.get();
                dbg_log(&format!("[LAYOUT] ChatLayout outer closure re-ran, initialized={}", init));
                if !init {
                    view! {
                        <div class="chat-loading">
                            <div class="chat-loading-spinner"></div>
                            <span>"Connecting to opman..."</span>
                        </div>
                    }.into_any()
                } else {
                    dbg_log("[LAYOUT] Creating ChatLayoutInner");
                    view! {
                        <ChatLayoutInner sse=sse panels=panels modal_state=modal_state mobile=mobile theme_mode=theme_mode set_theme_mode=set_theme_mode />
                    }.into_any()
                }
            }}
        </div>
    }
}

/// Inner layout rendered once app state is available.
#[component]
fn ChatLayoutInner(
    sse: SseState,
    panels: PanelState,
    modal_state: ModalState,
    mobile: MobileState,
    theme_mode: ReadSignal<ThemeMode>,
    set_theme_mode: WriteSignal<ThemeMode>,
) -> impl IntoView {
    dbg_log("[LAYOUT] ChatLayoutInner constructor called");
    let mobile_sidebar_open = mobile.sidebar_open;

    // ── Provider + model state ──
    let providers = use_providers();
    let model_state = use_model_state(sse.messages, providers);
    provide_context(providers); // Make providers available to modals (RoutinesModal etc.)

    // ── Assistant state ──
    let assistant = use_assistant_state(sse.app_state, move || sse.active_session_id());

    // ── Bookmarks ──
    let bookmarks = use_bookmarks();
    provide_context(bookmarks);

    // ── Virtual keyboard (mobile) ──
    let vkb_open = use_virtual_keyboard();
    provide_context(vkb_open);

    // ── URL restore & sync ──
    let url_state = use_url_restore(sse, panels);

    // ── Back-navigation (browser/system back button) ──
    let _back_nav = use_back_navigation(modal_state, panels, mobile);

    // ── Notification signals ──
    let (assistant_signals, set_assistant_signals) =
        use_notification_signals(sse, assistant.autonomy_mode);

    // ── Toast context (already provided at App level, just use it) ──
    let toasts = expect_context::<crate::hooks::use_toast::ToastState>();

    // ── Cross-session persistent toasts for sub-session permissions/questions ──
    // Matches React ChatLayout.tsx crossToastMapRef + useEffect pattern.
    // Shows persistent (duration=0) toasts for permissions/questions from sub-sessions.
    {
        let toast_ctx = expect_context::<ToastContext>();
        let cross_toast_map: StoredValue<HashMap<String, u64>> =
            StoredValue::new(HashMap::new());
        let sse_inner = sse;

        // Use derived signals to avoid subscribing to full app_state.
        let derived_project = sse.derived_active_project;

        Effect::new(move |_| {
            // Read reactive sources so this effect re-runs when they change
            let perms = sse_inner.permissions.get();
            let questions = sse_inner.questions.get();
            let proj = derived_project.get();

            // Build the set of sub-session IDs (children of the active session)
            let active_session_id = proj.as_ref().and_then(|p| p.active_session.clone());

            let sub_session_ids: HashSet<String> = if let (Some(ref asid), Some(ref p)) =
                (&active_session_id, &proj)
            {
                p.sessions
                    .iter()
                    .filter(|s| s.parent_id == *asid)
                    .map(|s| s.id.clone())
                    .collect()
            } else {
                HashSet::new()
            };

            // Collect current cross-session request IDs from sub-sessions only
            let mut current_ids = HashSet::<String>::new();

            cross_toast_map.update_value(|map| {
                // Permissions from sub-sessions
                for perm in &perms {
                    if active_session_id
                        .as_ref()
                        .map_or(true, |asid| perm.session_id == *asid)
                    {
                        continue; // Active session — shown inline, not as toast
                    }
                    if !sub_session_ids.contains(&perm.session_id) {
                        continue; // Not a sub-session
                    }
                    current_ids.insert(perm.id.clone());
                    if !map.contains_key(&perm.id) {
                        let label = if perm.tool_name.is_empty() {
                            "Permission".to_string()
                        } else {
                            perm.tool_name.clone()
                        };
                        let tid = toast_ctx.add(
                            format!(
                                "**Permission request** from sub-session: *{}*",
                                label
                            ),
                            ToastType::Warning,
                            0, // persistent — never auto-dismissed
                        );
                        map.insert(perm.id.clone(), tid);
                    }
                }

                // Questions from sub-sessions
                for q in &questions {
                    if active_session_id
                        .as_ref()
                        .map_or(true, |asid| q.session_id == *asid)
                    {
                        continue; // Active session
                    }
                    if !sub_session_ids.contains(&q.session_id) {
                        continue; // Not a sub-session
                    }
                    current_ids.insert(q.id.clone());
                    if !map.contains_key(&q.id) {
                        let label = if q.title.is_empty() {
                            "Question".to_string()
                        } else {
                            q.title.clone()
                        };
                        let tid = toast_ctx.add(
                            format!("**Question** from sub-session: *{}*", label),
                            ToastType::Info,
                            0, // persistent
                        );
                        map.insert(q.id.clone(), tid);
                    }
                }

                // Remove toasts for resolved items
                let to_remove: Vec<String> = map
                    .keys()
                    .filter(|req_id| !current_ids.contains(*req_id))
                    .cloned()
                    .collect();
                for req_id in to_remove {
                    if let Some(toast_id) = map.remove(&req_id) {
                        toast_ctx.remove(toast_id);
                    }
                }
            });
        });
    }

    // ── Chat handlers ──
    let handlers = use_chat_handlers(ChatHandlerDeps {
        sse,
        panels,
        modals: modal_state,
        toasts,
        selected_model: model_state.selected_model,
        set_selected_model: model_state.set_selected_model,
        selected_agent: model_state.selected_agent,
        set_selected_agent: model_state.set_selected_agent,
        current_agent: model_state.current_agent,
        sending: model_state.sending,
        set_sending: model_state.set_sending,
        active_memory_items: assistant.active_memory_items,
    });

    // ── Chat callbacks ──
    let (search_match_ids, set_search_match_ids) = signal(Vec::<String>::new());
    let (active_search_match_id, set_active_search_match_id) = signal(Option::<String>::None);

    let callbacks = use_chat_callbacks(ChatCallbackDeps {
        sse,
        panels,
        toasts,
        personal_memory: assistant.personal_memory,
        set_autonomy_mode: assistant.set_autonomy_mode,
        set_assistant_signals,
        set_active_workspace_name: assistant.set_active_workspace_name,
        search_match_ids: set_search_match_ids,
        active_search_match_id: set_active_search_match_id,
        handle_select_session: handlers.handle_select_session.clone(),
    });

    // ── Pulse actions ──
    let pulse_actions = use_pulse_actions(
        assistant.assistant_pulse,
        move || sse.active_session_id(),
        modal_state,
        toasts,
        assistant.set_autonomy_mode,
        assistant.set_routine_cache,
        assistant.set_workspace_cache,
    );

    // ── A2UI callback listener ──
    // Listens for `opman:a2ui-callback` custom events dispatched by interactive
    // A2UI blocks (buttons / forms) and POSTs the callback to the backend,
    // which injects it into the current session as a user message.
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        let handler =
            Closure::<dyn Fn(web_sys::CustomEvent)>::new(move |evt: web_sys::CustomEvent| {
                let detail_js = evt.detail();
                let Some(detail_str) = detail_js.as_string() else {
                    return;
                };
                let Ok(detail) = serde_json::from_str::<serde_json::Value>(&detail_str) else {
                    return;
                };
                let Some(callback_id) = detail.get("callback_id").and_then(|v| v.as_str()) else {
                    return;
                };
                let payload = detail.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                let Some(session_id) = sse.tracked_session_id() else {
                    return;
                };
                let cb_id = callback_id.to_string();
                leptos::task::spawn_local(async move {
                    let _ = crate::api::session::a2ui_callback(&session_id, &cb_id, payload).await;
                });
            });
        let window = web_sys::window().unwrap();
        let _ = window.add_event_listener_with_callback(
            "opman:a2ui-callback",
            handler.as_ref().unchecked_ref(),
        );
        let handler_fn = handler.as_ref().unchecked_ref::<js_sys::Function>().clone();
        on_cleanup(move || {
            if let Some(w) = web_sys::window() {
                let _ = w
                    .remove_event_listener_with_callback("opman:a2ui-callback", &handler_fn);
            }
        });
        handler.forget();
    }

    // ── Provide extended context for child components ──
    provide_context(assistant);
    provide_context(model_state);
    provide_context(handlers.clone());
    provide_context(pulse_actions);

    view! {
        // Mobile sidebar overlay (React: direct child of chat-layout)
        {move || {
            if mobile_sidebar_open.get() {
                Some(view! {
                    <div
                        class="sidebar-overlay visible"
                        on:click=move |_| mobile.close_sidebar()
                    />
                })
            } else {
                None
            }
        }}

        // Main content area (React: direct child of chat-layout, no wrapper)
        <ChatMainArea sse=sse panels=panels mobile=mobile modal_state=modal_state />

        // Modal layer
        <ModalLayer
            sse=sse
            modal_state=modal_state
            panels=panels
            on_command=handlers.handle_command
            on_new_session=handlers.handle_new_session
            on_select_session=handlers.handle_select_session
            on_model_selected=handlers.handle_model_selected
            on_agent_change=handlers.handle_agent_change
            on_context_submit=callbacks.handle_context_submit
            on_theme_applied=callbacks.handle_theme_applied
            theme_mode=theme_mode
            set_theme_mode=set_theme_mode
        />

        // Status bar
        <StatusBar sse=sse panels=panels modal_state=modal_state />

        // Toast container (handled at app level)

        // Mobile dock
        <MobileDock mobile=mobile modal_state=modal_state panels=panels sse=sse />
    }
}
