//! PermissionDock — displays pending permission requests with allow/always/reject actions.
//! Matches React `PermissionDock.tsx`.

use crate::components::icons::*;
use crate::types::core::PermissionRequest;
use leptos::prelude::*;

/// PermissionDock component.
#[component]
pub fn PermissionDock(
    permissions: Memo<Vec<PermissionRequest>>,
    active_session_id: Memo<Option<String>>,
    on_reply: Callback<(String, String)>,
    on_go_to_session: Callback<String>,
) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(0usize);

    // Clamp active_tab when permissions list changes
    Effect::new(move |_| {
        let len = permissions.get().len();
        if active_tab.get_untracked() >= len && len > 0 {
            set_active_tab.set(len - 1);
        } else if len == 0 {
            set_active_tab.set(0);
        }
    });

    let show = Memo::new(move |_| !permissions.get().is_empty());

    view! {
        {move || {
            if !show.get() {
                return None;
            }

            let perms = permissions.get();
            let show_tabs = perms.len() > 1;
            let idx = active_tab.get().min(perms.len().saturating_sub(1));
            let active_perm = perms.get(idx).cloned();

            Some(view! {
                <div
                    class="permission-dock mx-4 mb-2 rounded-lg border border-warning/30 bg-bg-panel/95 backdrop-blur-sm shadow-lg overflow-hidden"
                    role="alertdialog"
                    aria-label="Permission requests"
                >
                    // Tab strip for multiple permissions
                    {if show_tabs {
                        let tabs = perms.iter().enumerate().map(|(i, perm)| {
                            let perm_id = perm.id.clone();
                            let tool = perm.tool_name.clone();
                            let is_cross = {
                                let asid = active_session_id.get_untracked();
                                asid.as_ref().map_or(false, |s| s != &perm.session_id)
                            };
                            let label = if tool.is_empty() { format!("Permission {}", i + 1) } else { tool };
                            let _ = perm_id;
                            view! {
                                <button
                                    class=move || {
                                        let base = "flex items-center gap-1 px-2.5 py-1 text-xs rounded-t transition-colors";
                                        if active_tab.get() == i {
                                            format!("{} bg-warning/20 text-warning border-b-2 border-warning", base)
                                        } else {
                                            format!("{} text-text-muted hover:text-text hover:bg-bg-hover", base)
                                        }
                                    }
                                    on:click=move |_| set_active_tab.set(i)
                                >
                                    <span class="text-warning">"!"</span>
                                    <span class="truncate max-w-[100px]">{label.clone()}</span>
                                    {if is_cross {
                                        Some(view! {
                                            <span class="text-[10px] px-1 rounded bg-primary/20 text-primary">"sub"</span>
                                        })
                                    } else {
                                        None
                                    }}
                                </button>
                            }
                        }).collect::<Vec<_>>();
                        Some(view! {
                            <div class="flex items-center gap-0.5 px-2 pt-1 border-b border-border-subtle overflow-x-auto">
                                {tabs}
                            </div>
                        })
                    } else {
                        None
                    }}

                    // Active permission card
                    {if let Some(perm) = active_perm {
                        let is_cross = {
                            let asid = active_session_id.get();
                            asid.as_ref().map_or(false, |s| s != &perm.session_id)
                        };
                        Some(view! {
                            <PermissionCard
                                perm=perm
                                is_cross_session=is_cross
                                on_reply=on_reply
                                on_go_to_session=Some(on_go_to_session)
                            />
                        })
                    } else {
                        None
                    }}
                </div>
            })
        }}
    }
}

/// Single permission card with details and action buttons.
#[component]
fn PermissionCard(
    perm: PermissionRequest,
    is_cross_session: bool,
    on_reply: Callback<(String, String)>,
    on_go_to_session: Option<Callback<String>>,
) -> impl IntoView {
    let perm_id = perm.id.clone();
    let perm_id_once = perm_id.clone();
    let perm_id_always = perm_id.clone();
    let perm_id_reject = perm_id.clone();
    let perm_id_key = perm_id.clone();

    let tool_name = perm.tool_name.clone();
    let description = perm.description.clone();
    let patterns = perm.patterns.clone();
    let metadata = perm.metadata.clone();
    let session_id_short = perm.session_id.chars().take(8).collect::<String>();

    // Auto-focus the "Allow Once" button when the card mounts (matches React PermissionDock)
    let allow_once_ref = NodeRef::<leptos::html::Button>::new();
    Effect::new(move |_| {
        if let Some(btn) = allow_once_ref.get() {
            let el: web_sys::HtmlElement = btn.into();
            gloo_timers::callback::Timeout::new(50, move || {
                let _ = el.focus();
            })
            .forget();
        }
    });

    view! {
        <div
            class="permission-card"
            tabindex="0"
            on:keydown=move |e| {
                use leptos::callback::Callable;
                let key = e.key();
                if key == "Enter" {
                    e.prevent_default();
                    on_reply.run((perm_id_key.clone(), "once".to_string()));
                } else if key == "a" || key == "A" {
                    e.prevent_default();
                    on_reply.run((perm_id_key.clone(), "always".to_string()));
                } else if key == "Escape" || key == "r" || key == "R" {
                    e.prevent_default();
                    on_reply.run((perm_id_key.clone(), "reject".to_string()));
                }
            }
        >
            // Header
            <div class="flex items-center gap-2 px-3 py-2 border-b border-border-subtle">
                <span class="text-warning font-bold">"!"</span>
                <span class="text-sm font-medium text-text">"Permission Required"</span>
                {if is_cross_session {
                    Some(view! {
                        <span class="permission-badge-subagent text-[10px] px-1.5 py-0.5 rounded bg-primary/20 text-primary font-medium">"subagent"</span>
                    })
                } else {
                    None
                }}
                {if !perm.session_id.is_empty() {
                    if let Some(go) = on_go_to_session {
                        let sid = perm.session_id.clone();
                        let sid_short = session_id_short.clone();
                        Some(view! {
                            <button
                                class="dock-session-link flex items-center gap-0.5 text-[10px] text-text-muted hover:text-primary transition-colors"
                                on:click=move |e: web_sys::MouseEvent| {
                                    e.stop_propagation();
                                    go.run(sid.clone());
                                }
                                title=format!("Go to session {}", sid_short)
                                aria-label="Go to session"
                            >
                                <IconExternalLink size=11 />
                                <span>{sid_short.clone()}</span>
                            </button>
                        }.into_any())
                    } else {
                        Some(view! {
                            <span class="text-[10px] text-text-muted ml-1">{session_id_short.clone()}</span>
                        }.into_any())
                    }
                } else {
                    None
                }}
                <span class="flex-1" />
                <span class="text-[10px] text-text-muted hidden sm:inline">
                    "Enter = allow \u{00b7} A = always \u{00b7} Esc = reject"
                </span>
            </div>

            // Body
            <div class="px-3 py-2 space-y-1.5">
                <div class="text-sm font-mono text-primary">
                    {if tool_name.is_empty() { "Unknown permission".to_string() } else { tool_name }}
                </div>
                {description.map(|d| view! {
                    <div class="text-xs text-text-muted">{d}</div>
                })}
                {patterns.filter(|p| !p.is_empty()).map(|pats| {
                    let items = pats.into_iter().map(|p| view! {
                        <code class="inline-block px-1.5 py-0.5 rounded bg-bg-hover text-xs font-mono text-text-muted">{p}</code>
                    }).collect::<Vec<_>>();
                    view! {
                        <div class="flex flex-wrap gap-1">{items}</div>
                    }
                })}
                {metadata.filter(|m| !m.is_empty()).map(|m| {
                    let json = serde_json::to_string_pretty(&m).unwrap_or_default();
                    view! {
                        <pre class="text-[10px] text-text-muted bg-bg-hover rounded p-1.5 overflow-auto max-h-24 font-mono">{json}</pre>
                    }
                })}
            </div>

            // Actions
            <div class="flex items-center gap-2 px-3 py-2 border-t border-border-subtle">
                <button
                    node_ref=allow_once_ref
                    class="flex items-center gap-1 px-3 py-1.5 rounded text-xs font-medium bg-success/20 text-success hover:bg-success/30 transition-colors"
                    on:click=move |_| {
                        use leptos::callback::Callable;
                        on_reply.run((perm_id_once.clone(), "once".to_string()));
                    }
                >
                    <span>"+"</span>
                    "Allow Once"
                </button>
                <button
                    class="flex items-center gap-1 px-3 py-1.5 rounded text-xs font-medium bg-primary/20 text-primary hover:bg-primary/30 transition-colors"
                    on:click=move |_| {
                        use leptos::callback::Callable;
                        on_reply.run((perm_id_always.clone(), "always".to_string()));
                    }
                >
                    <span>"++"</span>
                    "Always Allow"
                </button>
                <button
                    class="flex items-center gap-1 px-3 py-1.5 rounded text-xs font-medium bg-error/20 text-error hover:bg-error/30 transition-colors"
                    on:click=move |_| {
                        use leptos::callback::Callable;
                        on_reply.run((perm_id_reject.clone(), "reject".to_string()));
                    }
                >
                    <span>"x"</span>
                    "Reject"
                </button>
            </div>
        </div>
    }
}
