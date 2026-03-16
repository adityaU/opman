//! StatusBar — bottom bar with project info, toggles, and stats.
//! Matches React `StatusBar.tsx` + `WatcherStatusBar.tsx`.

use leptos::prelude::*;
use leptos::reactive::owner::LocalStorage;

use crate::components::icons::*;
use crate::hooks::use_assistant_state::AssistantState;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_model_state::ModelState;
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_pulse_actions::PulseActions;
use crate::hooks::use_sse_state::{ConnectionStatus, SessionStatus, SseState};

/// Format a token count for compact display.
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format autonomy mode for display (matches React `formatAutonomyMode`).
fn format_autonomy_mode(mode: &str) -> &'static str {
    match mode {
        "observe" => "Observe",
        "nudge" => "Nudge",
        "continue" => "Continue",
        "autonomous" => "Auto",
        _ => "Observe",
    }
}

/// StatusBar component.
#[component]
pub fn StatusBar(sse: SseState, panels: PanelState, modal_state: ModalState) -> impl IntoView {
    let sidebar_open = panels.sidebar_open;
    let terminal_open = panels.terminal.open;
    let editor_open = panels.editor.open;
    let git_open = panels.git.open;
    let session_status = sse.session_status;
    let connection_status = sse.connection_status;

    // Consume AssistantState, ModelState, PulseActions from context
    let assistant = use_context::<AssistantState>();
    let model_state = use_context::<ModelState>();
    let pulse_actions = use_context::<PulseActions>();

    let active_project = Memo::new(move |_| sse.active_project());
    let project_name = Memo::new(move |_| {
        active_project
            .get()
            .map(|p| p.name.clone())
            .unwrap_or_default()
    });
    let git_branch = Memo::new(move |_| {
        active_project.get().and_then(|p| {
            if p.git_branch.is_empty() {
                None
            } else {
                Some(p.git_branch.clone())
            }
        })
    });

    let watcher_status = sse.watcher_status;

    // Watcher local tick for smooth countdown.
    // The interval handle is stored so old intervals are canceled when
    // watcher_status changes (prevents accumulating leaked timers).
    let (local_idle_secs, set_local_idle_secs) = signal::<Option<u64>>(None);
    let interval_handle: StoredValue<Option<gloo_timers::callback::Interval>, LocalStorage> =
        StoredValue::new_local(None);
    Effect::new(move |_| {
        let ws = watcher_status.get();
        // Cancel any previous interval before starting a new one.
        interval_handle.update_value(|h: &mut Option<gloo_timers::callback::Interval>| {
            h.take();
        });

        match ws {
            Some(status) if status.action == "countdown" => {
                if let Some(secs) = status.idle_since_secs {
                    set_local_idle_secs.set(Some(secs));
                    let handle = gloo_timers::callback::Interval::new(1_000, move || {
                        set_local_idle_secs.update(|v| {
                            if let Some(s) = v {
                                *s += 1;
                            }
                        });
                    });
                    interval_handle.set_value(Some(handle));
                } else {
                    set_local_idle_secs.set(status.idle_since_secs);
                }
            }
            Some(status) => {
                set_local_idle_secs.set(status.idle_since_secs);
            }
            None => {
                set_local_idle_secs.set(None);
            }
        }
    });

    let stats = sse.stats;
    let total_tokens = Memo::new(move |_| {
        stats
            .get()
            .map(|s| {
                s.input_tokens + s.output_tokens + s.reasoning_tokens + s.cache_read + s.cache_write
            })
            .unwrap_or(0)
    });
    let cost = Memo::new(move |_| stats.get().map(|s| s.cost).unwrap_or(0.0));

    // Context limit from ModelState
    let context_limit =
        Memo::new(move |_| model_state.and_then(|ms| ms.current_model_context_limit.get()));

    // Context percentage and color class (matches React logic)
    let context_pct = Memo::new(move |_| {
        let limit = context_limit.get();
        let tokens = total_tokens.get();
        match limit {
            Some(l) if l > 0 && tokens > 0 => {
                Some(((tokens as f64 / l as f64) * 100.0).round() as u64)
            }
            _ => None,
        }
    });

    let context_color_class = Memo::new(move |_| match context_pct.get() {
        Some(pct) if pct > 90 => "context-critical",
        Some(pct) if pct > 70 => "context-warning",
        Some(_) => "context-ok",
        None => "",
    });

    // Assistant-derived signals
    let active_workspace_name =
        Memo::new(move |_| assistant.and_then(|a| a.active_workspace_name.get()));
    let active_memory_items = Memo::new(move |_| {
        assistant
            .map(|a| a.active_memory_items.get())
            .unwrap_or_default()
    });
    let autonomy_mode =
        Memo::new(move |_| assistant.map(|a| a.autonomy_mode.get()).unwrap_or_default());
    let assistant_pulse = Memo::new(move |_| assistant.and_then(|a| a.assistant_pulse.get()));

    view! {
        <div class="chat-status-bar">
            // Left section
            <div class="status-bar-left">
                // Toggle sidebar
                <button
                    class=move || {
                        let base = "status-bar-btn";
                        if sidebar_open.get() { format!("{} active", base) } else { base.to_string() }
                    }
                    on:click=move |_| panels.toggle_sidebar()
                    title="Toggle Sidebar (Cmd+B)"
                    aria-label="Toggle sidebar"
                >
                    <IconPanelLeft size=13 />
                </button>

                // Project name
                {move || {
                    let name = project_name.get();
                    if !name.is_empty() {
                        Some(view! { <span class="status-bar-project">{name}</span> })
                    } else {
                        None
                    }
                }}

                // Git branch
                {move || {
                    git_branch.get().map(|branch| view! {
                        <span class="status-bar-branch">
                            <IconGitBranch size=11 />
                            {branch}
                        </span>
                    })
                }}

                // Status dot + label
                <span
                    class=move || {
                        let base = "status-bar-dot";
                        if session_status.get() == SessionStatus::Busy {
                            format!("{} busy", base)
                        } else {
                            format!("{} idle", base)
                        }
                    }
                    role="status"
                    aria-label=move || {
                        if session_status.get() == SessionStatus::Busy {
                            "Session is busy"
                        } else {
                            "Session is ready"
                        }
                    }
                />
                <span class="status-bar-status">
                    {move || if session_status.get() == SessionStatus::Busy { "busy" } else { "ready" }}
                </span>

                // Connection status (only when not connected)
                {move || {
                    let status = connection_status.get();
                    if status != ConnectionStatus::Connected {
                        Some(view! {
                            <span
                                class=move || format!("status-bar-connection status-bar-connection-{}", status.as_str())
                                role="status"
                                aria-label=move || {
                                    if status == ConnectionStatus::Reconnecting {
                                        "Reconnecting to server"
                                    } else {
                                        "Disconnected from server"
                                    }
                                }
                                title=move || {
                                    if status == ConnectionStatus::Reconnecting {
                                        "Reconnecting..."
                                    } else {
                                        "Disconnected"
                                    }
                                }
                            >
                                <IconWifiOff size=11 />
                                <span>{status.as_str()}</span>
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                // Watcher status indicator
                {move || {
                    let ws = watcher_status.get();
                    ws.map(|status| {
                        let display_secs = local_idle_secs.get().unwrap_or(status.idle_since_secs.unwrap_or(0));

                        let (dot_class, label): (&str, String) = match status.action.as_str() {
                            "countdown" => {
                                ("watcher-dot watcher-dot-green", format!("idle {}s -- continuing soon", display_secs))
                            }
                            "triggered" => ("watcher-dot watcher-dot-yellow", "running".to_string()),
                            "created" => ("watcher-dot watcher-dot-muted", "watching".to_string()),
                            "cancelled" => ("watcher-dot watcher-dot-muted", "cancelled".to_string()),
                            other => ("watcher-dot watcher-dot-muted", other.to_string()),
                        };
                        let dot_class = dot_class.to_string();
                        view! {
                            <button
                                class="watcher-status-indicator"
                                on:click=move |_| modal_state.open(ModalName::Watcher)
                                title="Session Watcher (Cmd+Shift+W)"
                            >
                                <span class=dot_class />
                                <IconEye size=11 />
                                <span class="watcher-status-label">{label}</span>
                            </button>
                        }
                    })
                }}

                // Presence badge (React: presenceClients.length > 1)
                {move || {
                    let clients = sse.presence_clients.get();
                    if clients.len() > 1 {
                        let count = clients.len();
                        let types = clients
                            .iter()
                            .map(|c| c.interface_type.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let title = format!(
                            "{} connected client{}: {}",
                            count,
                            if count != 1 { "s" } else { "" },
                            types
                        );
                        Some(view! {
                            <span class="status-bar-presence" title=title>
                                <IconUsers size=11 />
                                <span>{count}</span>
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                // Workspace badge
                {move || {
                    active_workspace_name.get().map(|name| {
                        let title = format!("Workspace: {}", name);
                        view! {
                            <span class="status-bar-workspace" title=title>
                                <IconLayers size=11 />
                                <span>{name}</span>
                            </span>
                        }
                    })
                }}

                // Memory badge
                {move || {
                    let items = active_memory_items.get();
                    if !items.is_empty() {
                        let title = items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>().join(", ");
                        let count = items.len();
                        Some(view! {
                            <span class="status-bar-memory" title=title>
                                <IconBrain size=11 />
                                <span>{count}</span>
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                // Autonomy badge
                {move || {
                    let mode = autonomy_mode.get();
                    if !mode.is_empty() {
                        let display = format_autonomy_mode(&mode);
                        let class = format!("status-bar-autonomy status-bar-autonomy-{}", mode);
                        Some(view! {
                            <span class=class>
                                <IconBot size=11 />
                                <span>{display}</span>
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                // Assistant pulse button
                {
                    let pulse_actions = pulse_actions.clone();
                    move || {
                        let pulse = assistant_pulse.get();
                        let has_actions = pulse_actions.is_some();
                        pulse.filter(|_| has_actions).map(|p| {
                            let class = format!("status-bar-pulse status-bar-pulse-{}", p.priority);
                            let title = p.rationale.clone();
                            let pulse_title = p.title.clone();
                            let pa = pulse_actions.clone();
                            view! {
                                <button
                                    class=class
                                    on:click=move |_| {
                                        if let Some(ref pa) = pa {
                                            pa.handle_run_assistant_pulse.run(());
                                        }
                                    }
                                    title=title
                                >
                                    <IconBot size=11 />
                                    <span>{pulse_title}</span>
                                </button>
                            }
                        })
                    }
                }
            </div>

            // Right section
            <div class="status-bar-right">
                // Token count with context window — color coded
                {move || {
                    let t = total_tokens.get();
                    if t > 0 {
                        let color_class = context_color_class.get();
                        let btn_class = if color_class.is_empty() {
                            "status-bar-tokens status-bar-tokens-btn".to_string()
                        } else {
                            format!("status-bar-tokens status-bar-tokens-btn {}", color_class)
                        };

                        let limit = context_limit.get();
                        let pct = context_pct.get();

                        // Build title string matching React
                        let title_str = match (limit, pct) {
                            (Some(l), Some(p)) => format!("{} / {} tokens ({}%) \u{2014} Click for details", t, l, p),
                            _ => format!("{} tokens \u{2014} Click for details", t),
                        };

                        Some(view! {
                            <button
                                class=btn_class
                                title=title_str
                                on:click=move |_| modal_state.open(ModalName::ContextWindow)
                            >
                                <IconZap size=11 />
                                {format_tokens(t)}
                                {limit.map(|l| view! {
                                    <span class="status-bar-token-limit">{format!(" / {}", format_tokens(l))}</span>
                                })}
                                {pct.map(|p| view! {
                                    <span class="status-bar-token-pct">{format!(" {}%", p)}</span>
                                })}
                            </button>
                        })
                    } else {
                        None
                    }
                }}

                // Cost
                {move || {
                    let c = cost.get();
                    if c > 0.0 {
                        Some(view! {
                            <span class="status-bar-cost">
                                <IconDollarSign size=11 />
                                {format!("{:.4}", c)}
                            </span>
                        })
                    } else {
                        None
                    }
                }}

                // Editor toggle
                <button
                    class=move || {
                        let base = "status-bar-btn";
                        if editor_open.get() { format!("{} active", base) } else { base.to_string() }
                    }
                    on:click=move |_| panels.editor.toggle()
                    title="Toggle Editor (Cmd+Shift+E)"
                    aria-label="Toggle editor"
                >
                    <IconFileCode size=13 />
                </button>

                // Git toggle
                <button
                    class=move || {
                        let base = "status-bar-btn";
                        if git_open.get() { format!("{} active", base) } else { base.to_string() }
                    }
                    on:click=move |_| panels.git.toggle()
                    title="Toggle Git (Cmd+Shift+G)"
                    aria-label="Toggle git"
                >
                    <IconGitBranch size=13 />
                </button>

                // Terminal toggle
                <button
                    class=move || {
                        let base = "status-bar-btn";
                        if terminal_open.get() { format!("{} active", base) } else { base.to_string() }
                    }
                    on:click=move |_| panels.terminal.toggle()
                    title="Toggle Terminal (Cmd+`)"
                    aria-label="Toggle terminal"
                >
                    <IconTerminal size=13 />
                </button>

                // Command palette
                <button
                    class="status-bar-btn"
                    on:click=move |_| modal_state.open(ModalName::CommandPalette)
                    title="Command Palette (Cmd+Shift+P)"
                    aria-label="Command palette"
                >
                    <IconCommand size=13 />
                </button>
            </div>
        </div>
    }
}
