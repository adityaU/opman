//! SplitView — side-by-side dual session view.
//! Matches React `SplitView.tsx`.
//! Note: This is a simplified port — the full dual-message-timeline + prompt
//! requires deeper integration with MessageTimeline and PromptInput components.
//! This implementation provides the structural layout and session picker.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::SessionInfo;
use leptos::prelude::*;

/// SplitView component.
#[component]
pub fn SplitView(
    primary_session_id: Option<String>,
    secondary_session_id: ReadSignal<Option<String>>,
    set_secondary_session_id: WriteSignal<Option<String>>,
    on_close: Callback<()>,
    sessions: Vec<SessionInfo>,
) -> impl IntoView {
    let (picker_open, set_picker_open) = signal(false);

    let primary_title = {
        let sessions_clone = sessions.clone();
        let pid = primary_session_id.clone();
        move || {
            pid.as_ref()
                .and_then(|id| {
                    sessions_clone.iter().find(|s| &s.id == id).map(|s| {
                        if s.title.is_empty() {
                            s.id.chars().take(8).collect()
                        } else {
                            s.title.clone()
                        }
                    })
                })
                .unwrap_or_else(|| "No session".into())
        }
    };

    let sessions_for_secondary = sessions.clone();
    let sessions_for_picker = sessions.clone();
    let primary_id = primary_session_id.clone();

    let filtered_sessions = {
        let primary_id = primary_id.clone();
        let sessions_for_secondary = sessions_for_secondary.clone();
        move || {
            let sec = secondary_session_id.get();
            sessions_for_secondary
                .iter()
                .filter(|s| Some(&s.id) != primary_id.as_ref() && Some(&s.id) != sec.as_ref())
                .cloned()
                .collect::<Vec<_>>()
        }
    };

    // Clone for the second usage of filtered_sessions
    let filtered_sessions2 = filtered_sessions.clone();

    let secondary_title = {
        let sessions_clone = sessions_for_picker.clone();
        move || {
            secondary_session_id
                .get()
                .and_then(|id| {
                    sessions_clone.iter().find(|s| s.id == id).map(|s| {
                        if s.title.is_empty() {
                            s.id.chars().take(8).collect()
                        } else {
                            s.title.clone()
                        }
                    })
                })
                .unwrap_or_else(|| "Select session".into())
        }
    };

    view! {
        <div class="split-view" style="display: flex; height: 100%; overflow: hidden;">
            // Left pane
            <div class="split-view-pane split-view-left" style="width: 50%; flex-shrink: 0; display: flex; flex-direction: column; overflow: hidden; border-right: 1px solid var(--color-border);">
                <div class="split-view-pane-header">
                    <span class="split-view-pane-title">
                        <svg class="w-2 h-2" viewBox="0 0 24 24" fill="var(--color-success)" stroke="none"><circle cx="12" cy="12" r="12"/></svg>
                        " " {primary_title}
                    </span>
                </div>
                <div class="split-view-pane-body" style="flex: 1; overflow: auto; padding: 16px;">
                    <div class="split-view-placeholder">"Primary session messages would render here."</div>
                </div>
            </div>

            // Right pane
            <div class="split-view-pane split-view-right" style="flex: 1; display: flex; flex-direction: column; overflow: hidden;">
                <div class="split-view-pane-header">
                    <span class="split-view-pane-title" style="position: relative;">
                        <svg class="w-2 h-2" viewBox="0 0 24 24" fill="var(--color-success)" stroke="none"><circle cx="12" cy="12" r="12"/></svg>
                        " "
                        <button class="split-view-picker-btn" on:click=move |_| set_picker_open.update(|v| *v = !*v)
                            style="background: none; border: none; color: inherit; cursor: pointer; display: inline-flex; align-items: center; gap: 4px;">
                            {secondary_title}
                            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
                        </button>
                {move || if picker_open.get() {
                            let filtered = filtered_sessions();
                            Some(view! {
                                <div class="split-view-picker" style="position: absolute; top: 100%; left: 0; z-index: 100; background: var(--color-bg); border: 1px solid var(--color-border); border-radius: 6px; max-height: 200px; overflow-y: auto;">
                                    {if filtered.is_empty() {
                                        view! { <div class="split-view-picker-empty" style="padding: 8px 12px; opacity: 0.6;">"No other sessions"</div> }.into_any()
                                    } else {
                                        filtered.into_iter().map(|s| {
                                            let id = s.id.clone();
                                            let title = if s.title.is_empty() { s.id.chars().take(8).collect() } else { s.title.clone() };
                                            view! {
                                                <button class="split-view-picker-item"
                                                    style="display: block; width: 100%; text-align: left; padding: 6px 12px; border: none; background: none; color: inherit; cursor: pointer;"
                                                    on:click=move |_| {
                                                        set_secondary_session_id.set(Some(id.clone()));
                                                        set_picker_open.set(false);
                                                    }>
                                                    {title}
                                                </button>
                                            }
                                        }).collect_view().into_any()
                                    }}
                                </div>
                            })
                        } else { None }}
                    </span>
                    <button class="split-view-close-btn" on:click=move |_| on_close.run(())
                        title="Close split view"
                        style="background: none; border: none; color: inherit; cursor: pointer; margin-left: auto;">
                        <IconX size=16 class="w-4 h-4" />
                    </button>
                </div>

                {move || {
                    if secondary_session_id.get().is_some() {
                        view! {
                            <div class="split-view-pane-body" style="flex: 1; overflow: auto; padding: 16px;">
                                <div class="split-view-placeholder">"Secondary session messages would render here."</div>
                            </div>
                        }.into_any()
                    } else {
                        let filtered = filtered_sessions2();
                        view! {
                            <div class="split-view-picker" style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px;">
                                <p style="opacity: 0.6;">"Select a session to compare"</p>
                                {filtered.into_iter().map(|s| {
                                    let id = s.id.clone();
                                    let title = if s.title.is_empty() { s.id.chars().take(8).collect() } else { s.title.clone() };
                                    view! {
                                        <button class="split-view-picker-item"
                                            style="padding: 6px 16px; border: 1px solid var(--color-border); border-radius: 6px; background: none; color: inherit; cursor: pointer;"
                                            on:click=move |_| set_secondary_session_id.set(Some(id.clone()))>
                                            {title}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
