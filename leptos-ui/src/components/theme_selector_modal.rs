//! ThemeSelectorModal — browse themes with live preview.
//! Matches React `ThemeSelectorModal.tsx`.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{ThemeColors, ThemePreview};
use crate::components::icons::*;

// ── Theme mode ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ThemeMode {
    Glassy,
    Flat,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::Glassy => "glassy",
            ThemeMode::Flat => "flat",
        }
    }
}

pub fn get_persisted_theme_mode() -> ThemeMode {
    let window = web_sys::window().expect("window");
    if let Ok(Some(storage)) = window.local_storage() {
        if let Ok(Some(val)) = storage.get_item("opman-theme-mode") {
            if val == "flat" {
                return ThemeMode::Flat;
            }
        }
    }
    ThemeMode::Glassy
}

pub fn persist_theme_mode(mode: ThemeMode) {
    let window = web_sys::window().expect("window");
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("opman-theme-mode", mode.as_str());
    }
}

pub fn apply_theme_mode(mode: ThemeMode) {
    let window = web_sys::window().expect("window");
    if let Some(doc) = window.document() {
        if let Some(el) = doc.document_element() {
            let class_list = el.class_list();
            match mode {
                ThemeMode::Flat => { let _ = class_list.add_1("flat-theme"); }
                ThemeMode::Glassy => { let _ = class_list.remove_1("flat-theme"); }
            }
        }
    }
}

fn apply_theme_to_css(colors: &ThemeColors) {
    crate::theme::apply_theme_to_css(colors);
}

fn save_current_css_vars() -> std::collections::HashMap<String, String> {
    let mut saved = std::collections::HashMap::new();
    let props = [
        "--color-primary", "--color-secondary", "--color-accent",
        "--color-bg", "--color-bg-panel", "--color-bg-element",
        "--color-text", "--color-text-muted",
        "--color-border", "--color-border-active", "--color-border-subtle",
        "--color-error", "--color-warning", "--color-success", "--color-info",
    ];
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        if let Ok(style) = web_sys::window().unwrap().get_computed_style(&el) {
            if let Some(cs) = style {
                for prop in &props {
                    if let Ok(val) = cs.get_property_value(prop) {
                        if !val.is_empty() {
                            saved.insert(prop.to_string(), val);
                        }
                    }
                }
            }
        }
    }
    saved
}

fn restore_css_vars(saved: &std::collections::HashMap<String, String>) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
            let style = html_el.style();
            for (prop, val) in saved {
                let _ = style.set_property(prop, val);
            }
        }
    }
}

// ── Component ───────────────────────────────────────────────────────

/// Theme selector modal component.
#[component]
pub fn ThemeSelectorModal(
    on_close: Callback<()>,
    on_theme_applied: Callback<ThemeColors>,
    theme_mode: ReadSignal<ThemeMode>,
    set_theme_mode: WriteSignal<ThemeMode>,
) -> impl IntoView {
    let (themes, set_themes) = signal::<Vec<ThemePreview>>(Vec::new());
    let (loading, set_loading) = signal(true);
    let (filter, set_filter) = signal(String::new());
    let (selected_idx, set_selected_idx) = signal(0usize);
    let (applying, set_applying) = signal(false);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Save original theme state for revert
    let original_vars = std::rc::Rc::new(std::cell::RefCell::new(save_current_css_vars()));
    let original_mode = std::rc::Rc::new(std::cell::RefCell::new(get_persisted_theme_mode()));

    // Fetch themes
    {
        leptos::task::spawn_local(async move {
            match crate::api::api_fetch::<Vec<ThemePreview>>("/themes").await {
                Ok(t) => {
                    set_themes.set(t);
                    set_loading.set(false);
                }
                Err(_) => {
                    set_loading.set(false);
                }
            }
        });
    }

    // Focus input
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    let filtered = Memo::new(move |_| {
        let all = themes.get();
        let q = filter.get().to_lowercase();
        if q.is_empty() {
            all
        } else {
            all.into_iter().filter(|t| t.name.to_lowercase().contains(&q)).collect()
        }
    });

    // Reset on filter change
    Effect::new(move |_| {
        let _ = filter.get();
        set_selected_idx.set(0);
    });

    // Live preview on selection change
    Effect::new(move |_| {
        let items = filtered.get();
        let idx = selected_idx.get();
        if let Some(theme) = items.get(idx) {
            apply_theme_to_css(&theme.colors);
        }
    });

    let orig_vars_revert = original_vars.clone();
    let orig_mode_revert = original_mode.clone();

    let revert_and_close = move || {
        restore_css_vars(&orig_vars_revert.borrow());
        let orig = *orig_mode_revert.borrow();
        if theme_mode.get_untracked() != orig {
            set_theme_mode.set(orig);
            apply_theme_mode(orig);
            persist_theme_mode(orig);
        }
        on_close.run(());
    };

    let revert_and_close2 = revert_and_close.clone();

    let handle_apply = move |theme: ThemePreview| {
        set_applying.set(true);
        let colors = theme.colors.clone();
        let name = theme.name.clone();
        leptos::task::spawn_local(async move {
            // Try to persist on server
            let _ = crate::api::api_post::<serde_json::Value>(
                "/theme/switch",
                &serde_json::json!({ "name": name }),
            ).await;
            // Apply locally regardless
            apply_theme_to_css(&colors);
            on_theme_applied.run(colors);
            on_close.run(());
        });
    };

    let handle_mode_switch = move |mode: ThemeMode| {
        set_theme_mode.set(mode);
        apply_theme_mode(mode);
        persist_theme_mode(mode);
    };

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected_idx.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected_idx.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                let idx = selected_idx.get_untracked();
                if let Some(theme) = items.get(idx).cloned() {
                    handle_apply(theme);
                }
            }
            "Escape" => {
                e.prevent_default();
                revert_and_close2();
            }
            _ => {}
        }
    };

    // Use a custom backdrop close that reverts
    let revert_close_backdrop = revert_and_close.clone();

    view! {
        <div
            class="modal-backdrop"
            on:click=move |_| revert_close_backdrop()
        >
            <div
                class="theme-selector"
                role="dialog"
                aria-modal="true"
                on:click=move |e| e.stop_propagation()
                on:keydown=on_keydown
            >
                <div class="theme-selector-header">
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/>
                        <circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/>
                        <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/>
                    </svg>
                    <span>"Appearance"</span>
                    <button class="theme-selector-close" on:click=move |_| revert_and_close()>
                        <IconX size=14 class="w-3.5 h-3.5" />
                    </button>
                </div>

                <div class="theme-mode-switcher">
                    <button
                        class=move || if theme_mode.get() == ThemeMode::Glassy { "theme-mode-option active" } else { "theme-mode-option" }
                        on:click=move |_| handle_mode_switch(ThemeMode::Glassy)
                    >
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/><rect width="7" height="7" x="3" y="3" rx="1"/>
                        </svg>
                        <div class="theme-mode-text">
                            <span class="theme-mode-label">"Glassy"</span>
                            <span class="theme-mode-desc">"Translucent blur effects"</span>
                        </div>
                    </button>
                    <button
                        class=move || if theme_mode.get() == ThemeMode::Flat { "theme-mode-option active" } else { "theme-mode-option" }
                        on:click=move |_| handle_mode_switch(ThemeMode::Flat)
                    >
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect width="18" height="18" x="3" y="3" rx="2"/>
                        </svg>
                        <div class="theme-mode-text">
                            <span class="theme-mode-label">"Flat"</span>
                            <span class="theme-mode-desc">"Solid opaque surfaces"</span>
                        </div>
                    </button>
                </div>

                <div class="theme-section-label">
                    <span>"Color Themes"</span>
                    <span class="theme-selector-count">{move || filtered.get().len()}</span>
                </div>

                <div class="theme-selector-search">
                    <IconSearch size=14 class="w-3.5 h-3.5" />
                    <input
                        class="theme-selector-input"
                        node_ref=input_ref
                        type="text"
                        placeholder="Search themes..."
                        prop:value=move || filter.get()
                        on:input=move |e| set_filter.set(event_target_value(&e))
                    />
                </div>

                <div class="theme-selector-grid">
                    {move || {
                        if loading.get() {
                            view! {
                                <div class="theme-selector-loading">
                                    <IconLoader2 size=16 class="w-4 h-4 spinning" />
                                    <span>"Loading themes..."</span>
                                </div>
                            }.into_any()
                        } else {
                            let items = filtered.get();
                            let sel = selected_idx.get();
                            if items.is_empty() {
                                view! { <div class="theme-selector-empty">"No themes found"</div> }.into_any()
                            } else {
                                items.into_iter().enumerate().map(|(idx, theme)| {
                                    let is_selected = idx == sel;
                                    let class_str = if is_selected { "theme-card selected" } else { "theme-card" };
                                    let theme2 = theme.clone();

                                    view! {
                                        <button
                                            class=class_str
                                            on:click=move |_| handle_apply(theme2.clone())
                                            on:mouseenter=move |_| set_selected_idx.set(idx)
                                        >
                                            <div class="theme-card-preview">
                                                <span style=format!("background: {}; flex: 2", theme.colors.background)></span>
                                                <span style=format!("background: {}; flex: 1", theme.colors.primary)></span>
                                                <span style=format!("background: {}; flex: 1", theme.colors.secondary)></span>
                                                <span style=format!("background: {}; flex: 1", theme.colors.accent)></span>
                                                <span style=format!("background: {}; flex: 1", theme.colors.text)></span>
                                            </div>
                                            <span class="theme-card-name">{theme.name.clone()}</span>
                                            {if applying.get_untracked() && is_selected {
                                                Some(view! {
                                                    <IconLoader2 size=12 class="w-3 h-3 spinning" />
                                                })
                                            } else {
                                                None
                                            }}
                                        </button>
                                    }
                                }).collect_view().into_any()
                            }
                        }
                    }}
                </div>

                <div class="theme-selector-footer">
                    <kbd>"Up/Down"</kbd>" Navigate "
                    <kbd>"Enter"</kbd>" Apply "
                    <kbd>"Esc"</kbd>" Cancel"
                </div>
            </div>
        </div>
    }
}
