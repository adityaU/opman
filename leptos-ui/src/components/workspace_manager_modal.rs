//! WorkspaceManagerModal — save, restore, and manage workspace snapshots.
//! Matches React `workspace-manager-modal/WorkspaceManagerModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_fetch, api_post};
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{WorkspaceSnapshot, WorkspacesListResponse};

// ── Helpers ─────────────────────────────────────────────────────────

fn format_date(iso: &str) -> String {
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

fn event_target_checked(e: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}

// ── API body ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SaveWorkspaceBody {
    snapshot: WorkspaceSnapshot,
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn WorkspaceManagerModal(
    on_close: Callback<()>,
    on_restore: Callback<WorkspaceSnapshot>,
    #[prop(optional)]
    on_save_current: Option<Callback<(), WorkspaceSnapshot>>,
    #[prop(optional)]
    active_workspace_name: Option<String>,
) -> impl IntoView {
    let (workspaces, set_workspaces) = signal(Vec::<WorkspaceSnapshot>::new());
    let (loading, set_loading) = signal(true);
    let (save_name, set_save_name) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (save_as_recipe, set_save_as_recipe) = signal(false);
    let (recipe_description, set_recipe_description) = signal(String::new());
    let (recipe_next_action, set_recipe_next_action) = signal(String::new());

    // Load on mount
    {
        leptos::task::spawn_local(async move {
            match api_fetch::<WorkspacesListResponse>("/workspaces").await {
                Ok(resp) => set_workspaces.set(resp.workspaces),
                Err(e) => leptos::logging::warn!("Failed to load workspaces: {}", e),
            }
            set_loading.set(false);
        });
    }

    // Save handler
    let handle_save = move |_: web_sys::MouseEvent| {
        let name = save_name.get_untracked().trim().to_string();
        if name.is_empty() || saving.get_untracked() {
            return;
        }
        let on_save = match on_save_current {
            Some(cb) => cb,
            None => return,
        };
        set_saving.set(true);
        let is_recipe = save_as_recipe.get_untracked();
        let desc = recipe_description.get_untracked().trim().to_string();
        let next_action = recipe_next_action.get_untracked().trim().to_string();
        let mut snapshot = on_save.run(());
        snapshot.name = name;
        snapshot.created_at = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        snapshot.is_recipe = Some(is_recipe);
        snapshot.recipe_description = if is_recipe && !desc.is_empty() {
            Some(desc)
        } else {
            None
        };
        snapshot.recipe_next_action = if is_recipe && !next_action.is_empty() {
            Some(next_action)
        } else {
            None
        };
        leptos::task::spawn_local(async move {
            let body = SaveWorkspaceBody { snapshot };
            let _ = api_post::<serde_json::Value>("/workspaces", &body).await;
            // Reload workspaces
            if let Ok(resp) = api_fetch::<WorkspacesListResponse>("/workspaces").await {
                set_workspaces.set(resp.workspaces);
            }
            set_save_name.set(String::new());
            set_recipe_description.set(String::new());
            set_recipe_next_action.set(String::new());
            set_save_as_recipe.set(false);
            set_saving.set(false);
        });
    };

    let active_name = active_workspace_name.unwrap_or_default();

    view! {
        <ModalOverlay on_close=on_close class="workspace-modal">
            <div class="workspace-header">
                <div class="workspace-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                        <line x1="8" y1="21" x2="16" y2="21" />
                        <line x1="12" y1="17" x2="12" y2="21" />
                    </svg>
                    <h3>"Workspaces"</h3>
                    <span class="workspace-count">{move || workspaces.get().len()}</span>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close workspaces">
                    <IconX size=16 />
                </button>
            </div>

            <div class="workspace-scrollable">
                // Saved Workspaces list
                <div class="workspace-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="workspace-empty">"Loading workspaces..."</div> }.into_any();
                        }
                        let all = workspaces.get();
                        if all.is_empty() {
                            return view! {
                                <div class="workspace-empty">"No saved workspaces yet."</div>
                            }.into_any();
                        }

                        // Separate templates/recipes and regular workspaces
                        let templates: Vec<_> = all.iter().filter(|w| w.is_template).cloned().collect();
                        let recipes: Vec<_> = all.iter().filter(|w| !w.is_template && w.is_recipe == Some(true)).cloned().collect();
                        let saved: Vec<_> = all.iter().filter(|w| !w.is_template && w.is_recipe != Some(true)).cloned().collect();
                        let active_name_inner = active_name.clone();

                        let sections = vec![
                            ("Templates", templates),
                            ("Recipes", recipes),
                            ("Saved Workspaces", saved),
                        ];

                        let section_views = sections.into_iter().filter_map(|(section_label, items)| {
                            if items.is_empty() {
                                return None;
                            }
                            let rows = items.into_iter().map(|ws| {
                                let name = ws.name.clone();
                                let name_display = ws.name.clone();
                                let created = format_date(&ws.created_at);
                                let is_active = ws.name == active_name_inner;
                                let desc = ws.recipe_description.clone().unwrap_or_default();
                                let ws_restore = ws.clone();
                                let ws_name_del = ws.name.clone();
                                let is_template = ws.is_template;

                                view! {
                                    <div class=if is_active { "workspace-item workspace-item-active" } else { "workspace-item" }>
                                        <div class="workspace-item-main">
                                            <div class="workspace-item-row">
                                                <span class="workspace-item-name">{name_display}</span>
                                                {is_active.then(|| view! {
                                                    <span class="workspace-item-badge">"Active"</span>
                                                })}
                                            </div>
                                            {(!desc.is_empty()).then(|| {
                                                let d = desc.clone();
                                                view! { <div class="workspace-item-desc">{d}</div> }
                                            })}
                                            <div class="workspace-item-meta">
                                                <span>{created}</span>
                                            </div>
                                        </div>
                                        <div class="workspace-item-actions">
                                            <button
                                                class="workspace-restore-btn"
                                                on:click={
                                                    let snap = ws_restore.clone();
                                                    move |_: web_sys::MouseEvent| on_restore.run(snap.clone())
                                                }
                                            >
                                                "Restore"
                                            </button>
                                            {(!is_template).then(|| {
                                                let del_name = ws_name_del.clone();
                                                view! {
                                                    <button
                                                        class="workspace-delete-btn"
                                                        on:click=move |_: web_sys::MouseEvent| {
                                                            let dn = del_name.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let path = format!("/workspaces?name={}", dn);
                                                                if api_delete(&path).await.is_ok() {
                                                                    set_workspaces.update(|list| list.retain(|w| w.name != dn));
                                                                }
                                                            });
                                                        }
                                                        aria-label="Delete"
                                                    >
                                                        <IconTrash2 size=14 />
                                                    </button>
                                                }
                                            })}
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>();

                            Some(view! {
                                <section class="workspace-section">
                                    <div class="workspace-section-title">{section_label}</div>
                                    {rows}
                                </section>
                            })
                        }).collect_view();

                        view! { <div>{section_views}</div> }.into_any()
                    }}
                </div>
            </div>

            // Save current workspace form (only if on_save_current is provided)
            {on_save_current.map(|_| {
                let hs = handle_save.clone();
                view! {
                    <div class="workspace-mgr-save">
                        <div class="workspace-mgr-save-fields">
                            <input
                                type="text"
                                placeholder="Save current workspace as..."
                                prop:value=move || save_name.get()
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    set_save_name.set(v);
                                }
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    if ev.key() == "Enter" {
                                        let name = save_name.get_untracked().trim().to_string();
                                        if !name.is_empty() && !saving.get_untracked() {
                                            // Trigger save via synthetic click (reuse the handler)
                                            if let Some(cb) = on_save_current {
                                                set_saving.set(true);
                                                let is_recipe = save_as_recipe.get_untracked();
                                                let desc = recipe_description.get_untracked().trim().to_string();
                                                let next_action = recipe_next_action.get_untracked().trim().to_string();
                                                let mut snapshot = cb.run(());
                                                snapshot.name = name;
                                                snapshot.created_at = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
                                                snapshot.is_recipe = Some(is_recipe);
                                                snapshot.recipe_description = if is_recipe && !desc.is_empty() { Some(desc) } else { None };
                                                snapshot.recipe_next_action = if is_recipe && !next_action.is_empty() { Some(next_action) } else { None };
                                                leptos::task::spawn_local(async move {
                                                    let body = SaveWorkspaceBody { snapshot };
                                                    let _ = api_post::<serde_json::Value>("/workspaces", &body).await;
                                                    if let Ok(resp) = api_fetch::<WorkspacesListResponse>("/workspaces").await {
                                                        set_workspaces.set(resp.workspaces);
                                                    }
                                                    set_save_name.set(String::new());
                                                    set_recipe_description.set(String::new());
                                                    set_recipe_next_action.set(String::new());
                                                    set_save_as_recipe.set(false);
                                                    set_saving.set(false);
                                                });
                                            }
                                        }
                                    }
                                }
                            />
                            <div class="workspace-mgr-save-meta">
                                <label class="workspace-mgr-recipe-toggle">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || save_as_recipe.get()
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_save_as_recipe.set(checked);
                                        }
                                    />
                                    "Save as recipe"
                                </label>
                                <span class="workspace-mgr-item-desc">
                                    {move || {
                                        if save_as_recipe.get() {
                                            "Recipes become reusable launch presets with guidance."
                                        } else {
                                            "Save the current panel layout and reopen it later."
                                        }
                                    }}
                                </span>
                            </div>
                            {move || {
                                if save_as_recipe.get() {
                                    Some(view! {
                                        <input
                                            type="text"
                                            placeholder="Recipe description"
                                            prop:value=move || recipe_description.get()
                                            on:input=move |ev| set_recipe_description.set(event_target_value(&ev))
                                        />
                                        <input
                                            type="text"
                                            placeholder="Suggested next action"
                                            prop:value=move || recipe_next_action.get()
                                            on:input=move |ev| set_recipe_next_action.set(event_target_value(&ev))
                                        />
                                    })
                                } else {
                                    None
                                }
                            }}
                        </div>
                        <button
                            on:click=hs
                            disabled=move || save_name.get().trim().is_empty() || saving.get()
                            title=move || if save_as_recipe.get() { "Save recipe" } else { "Save workspace" }
                        >
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
                                <polyline points="17 21 17 13 7 13 7 21" />
                                <polyline points="7 3 7 8 15 8" />
                            </svg>
                            {move || if saving.get() { "Saving..." } else { "Save" }}
                        </button>
                    </div>
                }
            })}

            <div class="workspace-footer">
                <span class="workspace-footer-count">
                    {move || {
                        let count = workspaces.get().len();
                        format!("{} saved workspace{}", count, if count != 1 { "s" } else { "" })
                    }}
                </span>
            </div>
        </ModalOverlay>
    }
}
