//! SlashCommandPopover — command autocomplete menu for the prompt input.
//! Leptos port of `web-ui/src/SlashCommandPopover.tsx`.
//! Fixed: filter is now reactive, selected_index resets on filter change,
//! keyboard listener is properly cleaned up on unmount.

use leptos::prelude::*;
use crate::types::core::SlashCommand;

/// Built-in commands that are always available.
fn builtin_commands() -> Vec<SlashCommand> {
    vec![
        // Web-local commands
        SlashCommand { name: "new".into(), description: Some("Start a new session".into()), args: None },
        SlashCommand { name: "cancel".into(), description: Some("Cancel / abort the running session".into()), args: None },
        SlashCommand { name: "terminal".into(), description: Some("Toggle terminal panel".into()), args: None },
        // Built-in server commands
        SlashCommand { name: "model".into(), description: Some("Change the AI model".into()), args: Some("<model>".into()) },
        SlashCommand { name: "models".into(), description: Some("List available models".into()), args: None },
        SlashCommand { name: "theme".into(), description: Some("Change color theme".into()), args: Some("<theme>".into()) },
        SlashCommand { name: "compact".into(), description: Some("Compact conversation history".into()), args: None },
        SlashCommand { name: "undo".into(), description: Some("Undo last action".into()), args: None },
        SlashCommand { name: "redo".into(), description: Some("Redo last action".into()), args: None },
        SlashCommand { name: "fork".into(), description: Some("Fork current session".into()), args: None },
        SlashCommand { name: "share".into(), description: Some("Share session".into()), args: None },
        SlashCommand { name: "agent".into(), description: Some("Switch agent type".into()), args: Some("<agent>".into()) },
        SlashCommand { name: "clear".into(), description: Some("Clear conversation".into()), args: None },
        // Modal commands
        SlashCommand { name: "keys".into(), description: Some("Show keyboard shortcuts".into()), args: None },
        SlashCommand { name: "todos".into(), description: Some("Show session todos".into()), args: None },
        SlashCommand { name: "sessions".into(), description: Some("Search sessions across projects".into()), args: None },
        SlashCommand { name: "context".into(), description: Some("Send context to session".into()), args: None },
        SlashCommand { name: "settings".into(), description: Some("Open settings".into()), args: None },
        SlashCommand { name: "assistant-center".into(), description: Some("Open the assistant cockpit".into()), args: None },
        SlashCommand { name: "inbox".into(), description: Some("Open the assistant inbox".into()), args: None },
        SlashCommand { name: "missions".into(), description: Some("Open mission tracking".into()), args: None },
        SlashCommand { name: "memory".into(), description: Some("Open personal memory".into()), args: None },
        SlashCommand { name: "autonomy".into(), description: Some("Adjust assistant autonomy".into()), args: None },
        SlashCommand { name: "routines".into(), description: Some("Manage assistant routines".into()), args: None },
        SlashCommand { name: "delegation".into(), description: Some("Open delegation board".into()), args: None },
        SlashCommand { name: "workspaces".into(), description: Some("Open workspaces and recipes".into()), args: None },
        SlashCommand { name: "system".into(), description: Some("Open system monitor (htop)".into()), args: None },
    ]
}

/// Slash command popover — shows matching commands as the user types.
#[component]
pub fn SlashCommandPopover(
    #[prop(into)] filter: Signal<String>,
    on_select: Callback<String>,
    on_close: Callback<()>,
    #[prop(optional)] session_id: Option<String>,
) -> impl IntoView {
    let (selected_index, set_selected_index) = signal(0usize);
    let (api_commands, set_api_commands) = signal::<Vec<SlashCommand>>(vec![]);

    // Fetch API commands on mount
    {
        let _sid = session_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::client::api_fetch::<Vec<SlashCommand>>("/commands").await {
                Ok(cmds) => {
                    if !cmds.is_empty() {
                        set_api_commands.set(cmds);
                    }
                }
                Err(_) => {}
            }
        });
    }

    // Merge built-in + API commands, deduplicating by name (builtin wins)
    let commands = Memo::new(move |_| {
        let builtins = builtin_commands();
        let builtin_names: std::collections::HashSet<String> = builtins.iter().map(|c| c.name.clone()).collect();
        let api = api_commands.get();
        let mut merged = builtins;
        for cmd in api {
            if !builtin_names.contains(&cmd.name) {
                merged.push(cmd);
            }
        }
        merged
    });

    // Filtered commands — REACTIVE: reads filter signal on every change
    let filtered = Memo::new(move |_| {
        let cmds = commands.get();
        let f = filter.get();
        if f.is_empty() {
            return cmds;
        }
        let lf = f.to_lowercase();
        cmds.into_iter().filter(|c| {
            c.name.to_lowercase().contains(&lf) ||
            c.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&lf))
        }).collect::<Vec<_>>()
    });

    // Reset selected_index when filter changes
    Effect::new(move |_| {
        let _f = filter.get(); // track filter changes
        set_selected_index.set(0);
    });

    // Register keyboard handler
    // Note: We store the JS function reference for cleanup via on_cleanup.
    // JsValue is Send+Sync in wasm32 targets, so this works with Leptos.
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let on_sel = on_select.clone();
        let on_cls = on_close.clone();

        let closure = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            let key = e.key();
            match key.as_str() {
                "ArrowDown" => {
                    e.prevent_default();
                    set_selected_index.update(|i| {
                        let len = filtered.get_untracked().len();
                        if *i + 1 < len { *i += 1; }
                    });
                }
                "ArrowUp" => {
                    e.prevent_default();
                    set_selected_index.update(|i| {
                        if *i > 0 { *i -= 1; }
                    });
                }
                "Enter" | "Tab" => {
                    e.prevent_default();
                    let items = filtered.get_untracked();
                    let idx = selected_index.get_untracked();
                    if let Some(cmd) = items.get(idx) {
                        on_sel.run(cmd.name.clone());
                    }
                }
                "Escape" => {
                    on_cls.run(());
                }
                _ => {}
            }
        });

        // Get the JS function reference before forgetting the closure
        let js_fn: js_sys::Function = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();

        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            let _ = document.add_event_listener_with_callback_and_bool(
                "keydown",
                &js_fn,
                true,
            );
        }

        // Forget the Rust closure (prevents deallocation), but we hold js_fn for cleanup
        closure.forget();

        // Cleanup: remove listener when component unmounts
        // js_sys::Function is Send+Sync on wasm32, so on_cleanup accepts it
        on_cleanup(move || {
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                let _ = document.remove_event_listener_with_callback_and_bool(
                    "keydown",
                    &js_fn,
                    true,
                );
            }
        });
    }

    view! {
        {move || {
            let items = filtered.get();
            if items.is_empty() {
                return None;
            }
            Some(view! {
                <div class="slash-popover">
                    {items.iter().enumerate().map(|(idx, cmd)| {
                        let name = cmd.name.clone();
                        let desc = cmd.description.clone();
                        let args = cmd.args.clone();
                        let name_for_click = name.clone();
                        let on_sel = on_select.clone();
                        let set_idx = set_selected_index;

                        view! {
                            <button
                                class=move || {
                                    if selected_index.get() == idx {
                                        "slash-popover-item slash-popover-item-active"
                                    } else {
                                        "slash-popover-item"
                                    }
                                }
                                on:click=move |_: web_sys::MouseEvent| on_sel.run(name_for_click.clone())
                                on:mouseenter=move |_: web_sys::MouseEvent| set_idx.set(idx)
                            >
                                <span class="slash-popover-name">{format!("/{}", name)}</span>
                                {desc.map(|d| view! {
                                    <span class="slash-popover-desc">{d}</span>
                                })}
                                {args.map(|a| view! {
                                    <span class="slash-popover-args">{a}</span>
                                })}
                            </button>
                        }
                    }).collect_view()}
                </div>
            })
        }}
    }
}
