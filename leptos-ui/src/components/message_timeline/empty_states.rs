//! Empty-state components for the message timeline:
//! - WelcomeEmpty (no session selected)
//! - MessageShimmer (loading)
//! - NewSessionEmpty (new session, idle)

use crate::hooks::use_sse_state::SseState;
use leptos::prelude::*;

/// Shimmer loading placeholder — matches React `MessageShimmer`.
#[component]
pub fn MessageShimmer() -> impl IntoView {
    view! {
        <div class="message-shimmer" aria-label="Loading messages">
            <div class="shimmer-turn shimmer-user">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-55" />
                    <div class="shimmer-line shimmer-w-35" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-assistant">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-90" />
                    <div class="shimmer-line shimmer-w-75" />
                    <div class="shimmer-line shimmer-w-60" />
                    <div class="shimmer-line shimmer-w-45" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-user">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-40" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-assistant">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-80" />
                    <div class="shimmer-line shimmer-w-65" />
                    <div class="shimmer-line shimmer-w-50" />
                </div>
            </div>
        </div>
    }
}

/// Welcome empty state — shown when no session is selected.
#[component]
pub fn WelcomeEmpty() -> impl IntoView {
    view! {
        <div class="message-timeline-empty">
            <div class="message-timeline-welcome">
                <h2>"Welcome to OpenCode"</h2>
                <p>"Select a session from the sidebar or create a new one to start chatting."</p>
                <div class="message-timeline-shortcuts">
                    <kbd>"Cmd+Shift+N"</kbd>" New Session"
                    <kbd>"Cmd+Shift+P"</kbd>" Command Palette"
                    <kbd>"Cmd'"</kbd>" Model Picker"
                </div>
            </div>
        </div>
    }
}

/// New session empty state — shown when session exists but has no messages and is idle.
#[component]
pub fn NewSessionEmpty(
    sse: SseState,
    #[prop(optional)] on_send_prompt: Option<Callback<String>>,
) -> impl IntoView {
    let session_directory = Memo::new(move |_| {
        let path = sse
            .active_project()
            .map(|p| p.path.clone())
            .unwrap_or_default();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    });

    let stored_send = StoredValue::new(on_send_prompt);

    let prompts: &[(&str, &str)] = &[
        ("code", "Refactor the auth module to use JWT tokens"),
        ("bug", "Find and fix the memory leak in the worker pool"),
        ("lightbulb", "Add unit tests for the API endpoints"),
        ("message", "Explain the architecture of this project"),
    ];

    let prompt_views = prompts.iter().map(|(icon, text)| {
        let text_owned = text.to_string();
        let text_for_click = text.to_string();
        let icon_svg = match *icon {
            "code" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" />
                </svg>
            }.into_any(),
            "bug" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="m8 2 1.88 1.88M14.12 3.88 16 2M9 7.13v-1a3.003 3.003 0 1 1 6 0v1" />
                    <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6" />
                    <path d="M12 20v-9M6.53 9C4.6 8.8 3 7.1 3 5M6 13H2M3 21c0-2.1 1.7-3.9 3.8-4M20.97 5c0 2.1-1.6 3.8-3.5 4M22 13h-4M17.2 17c2.1.1 3.8 1.9 3.8 4" />
                </svg>
            }.into_any(),
            "lightbulb" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5" />
                    <path d="M9 18h6M10 22h4" />
                </svg>
            }.into_any(),
            _ => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                </svg>
            }.into_any(),
        };
        view! {
            <button
                class="new-session-prompt-card"
                on:click=move |_| {
                    stored_send.with_value(|cb| {
                        if let Some(ref cb) = cb {
                            cb.run(text_for_click.clone());
                        }
                    });
                }
            >
                {icon_svg}
                <span>{text_owned}</span>
            </button>
        }
    }).collect_view();

    view! {
        <div class="message-timeline-empty">
            <div class="message-timeline-welcome new-session-welcome">
                <h2>"New Session"</h2>

                <div class="new-session-info">
                    {move || {
                        let dir: Option<String> = session_directory.get();
                        dir.map(|d| {
                            let d2 = d.clone();
                            view! {
                                <div class="new-session-info-row">
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />
                                    </svg>
                                    <span class="new-session-directory" title=d2>{d}</span>
                                </div>
                            }
                        })
                    }}
                </div>

                <p>"Type a message below or try one of these:"</p>

                <div class="new-session-prompts">
                    {prompt_views}
                </div>

                <div class="message-timeline-shortcuts">
                    <kbd>"Cmd'"</kbd>" Model Picker"
                    <kbd>"Cmd+Shift+E"</kbd>" Editor"
                    <kbd>"Cmd+Shift+G"</kbd>" Git"
                </div>
            </div>
        </div>
    }
}
