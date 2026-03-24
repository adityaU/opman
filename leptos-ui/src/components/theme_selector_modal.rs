//! ThemeSelectorModal — browse themes with live preview + appearance toggle.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::types::api::{ThemeColors, ThemePreview};
use crate::components::icons::*;

#[derive(Clone, Copy, PartialEq)]
pub enum ThemeMode { Glassy, Flat }

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self { Self::Glassy => "glassy", Self::Flat => "flat" }
    }
}

pub fn get_persisted_theme_mode() -> ThemeMode {
    match ls_get("opman-theme-mode").as_deref() {
        Some("flat") => ThemeMode::Flat,
        _ => ThemeMode::Glassy,
    }
}

pub fn persist_theme_mode(mode: ThemeMode) { ls_set("opman-theme-mode", mode.as_str()); }

pub fn apply_theme_mode(mode: ThemeMode) {
    let Some(root) = doc_el() else { return };
    let cl = root.class_list();
    match mode {
        ThemeMode::Flat => { let _ = cl.add_1("flat-theme"); }
        ThemeMode::Glassy => { let _ = cl.remove_1("flat-theme"); }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Appearance { System, Light, Dark }

impl Appearance {
    pub fn as_str(&self) -> &'static str {
        match self { Self::System => "system", Self::Light => "light", Self::Dark => "dark" }
    }
    pub fn from_str(s: &str) -> Self {
        match s { "light" => Self::Light, "system" => Self::System, _ => Self::Dark }
    }
    pub fn resolve(&self) -> &'static str { crate::theme::resolve_appearance(self.as_str()) }
}

pub fn get_persisted_appearance() -> Appearance {
    Appearance::from_str(&crate::theme::get_appearance())
}

const CSS_PROPS: [&str; 15] = [
    "--color-primary", "--color-secondary", "--color-accent",
    "--color-bg", "--color-bg-panel", "--color-bg-element",
    "--color-text", "--color-text-muted", "--color-border",
    "--color-border-active", "--color-border-subtle",
    "--color-error", "--color-warning", "--color-success", "--color-info",
];

fn save_css_vars() -> std::collections::HashMap<String, String> {
    let mut saved = std::collections::HashMap::new();
    let Some(el) = doc_el() else { return saved };
    let Some(cs) = web_sys::window().unwrap().get_computed_style(&el).ok().flatten() else {
        return saved;
    };
    for p in &CSS_PROPS {
        if let Ok(v) = cs.get_property_value(p) {
            if !v.is_empty() { saved.insert(p.to_string(), v); }
        }
    }
    saved
}

fn restore_css_vars(saved: &std::collections::HashMap<String, String>) {
    let Some(el) = doc_el() else { return };
    if let Some(h) = el.dyn_ref::<web_sys::HtmlElement>() {
        let s = h.style();
        for (p, v) in saved { let _ = s.set_property(p, v); }
    }
}

fn doc_el() -> Option<web_sys::Element> { web_sys::window()?.document()?.document_element() }

fn ls_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

fn ls_set(key: &str, val: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = s.set_item(key, val);
    }
}

fn pick_colors(t: &ThemePreview, a: Appearance) -> &ThemeColors {
    if a.resolve() == "light" { &t.light } else { &t.dark }
}

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
    let (sel_idx, set_sel_idx) = signal(0usize);
    let (applying, _set_applying) = signal(false);
    let (appearance, set_appearance) = signal(get_persisted_appearance());
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let orig_vars = std::rc::Rc::new(std::cell::RefCell::new(save_css_vars()));
    let orig_mode = std::rc::Rc::new(std::cell::RefCell::new(get_persisted_theme_mode()));
    let orig_app = std::rc::Rc::new(std::cell::RefCell::new(get_persisted_appearance()));

    leptos::task::spawn_local(async move {
        if let Ok(t) = crate::api::api_fetch::<Vec<ThemePreview>>("/themes").await {
            set_themes.set(t);
        }
        set_loading.set(false);
    });

    Effect::new(move |_| { if let Some(el) = input_ref.get() { let _ = el.focus(); } });

    let filtered = Memo::new(move |_| {
        let all = themes.get();
        let q = filter.get().to_lowercase();
        if q.is_empty() { all }
        else { all.into_iter().filter(|t| t.name.to_lowercase().contains(&q)).collect() }
    });

    Effect::new(move |_| { let _ = filter.get(); set_sel_idx.set(0); });

    Effect::new(move |_| {
        let items = filtered.get();
        let idx = sel_idx.get();
        let app = appearance.get();
        if let Some(theme) = items.get(idx) {
            crate::theme::apply_theme_to_css(pick_colors(theme, app));
        }
    });

    let (ov, om, oa) = (orig_vars.clone(), orig_mode.clone(), orig_app.clone());
    let revert_and_close = move || {
        restore_css_vars(&ov.borrow());
        let m = *om.borrow();
        if theme_mode.get_untracked() != m {
            set_theme_mode.set(m);
            apply_theme_mode(m);
            persist_theme_mode(m);
        }
        crate::theme::set_appearance(oa.borrow().as_str());
        on_close.run(());
    };
    let (rc2, rc3) = (revert_and_close.clone(), revert_and_close.clone());

    let handle_apply = move |theme: ThemePreview| {
        let app = appearance.get_untracked();
        let colors = pick_colors(&theme, app).clone();
        let name = theme.name.clone();
        leptos::task::spawn_local(async move {
            let _ = crate::api::api_post::<serde_json::Value>(
                "/theme/switch", &serde_json::json!({ "name": name }),
            ).await;
            crate::theme::apply_theme_to_css(&colors);
            on_theme_applied.run(colors);
            on_close.run(());
        });
    };

    let handle_mode = move |m: ThemeMode| {
        set_theme_mode.set(m); apply_theme_mode(m); persist_theme_mode(m);
    };
    let handle_app = move |a: Appearance| {
        set_appearance.set(a); crate::theme::set_appearance(a.as_str());
    };

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        match e.key().as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 { set_sel_idx.update(|i| *i = (*i + 1).min(len - 1)); }
            }
            "ArrowUp" => { e.prevent_default(); set_sel_idx.update(|i| *i = i.saturating_sub(1)); }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                if let Some(t) = items.get(sel_idx.get_untracked()).cloned() { handle_apply(t); }
            }
            "Escape" => { e.prevent_default(); rc2(); }
            _ => {}
        }
    };

    view! {
        <div class="modal-backdrop" on:click=move |_| rc3()>
          <div class="theme-selector" role="dialog" aria-modal="true"
              on:click=move |e| e.stop_propagation() on:keydown=on_keydown>
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
            <div class="appearance-switcher">
              {[Appearance::System, Appearance::Light, Appearance::Dark].into_iter().map(|a| {
                let label = a.as_str();
                view! { <button class=move || if appearance.get() == a { "appearance-option active" } else { "appearance-option" }
                    on:click=move |_| handle_app(a)>{label}</button> }
              }).collect_view()}
            </div>
            <div class="theme-mode-switcher">
              <button class=move || if theme_mode.get() == ThemeMode::Glassy { "theme-mode-option active" } else { "theme-mode-option" }
                  on:click=move |_| handle_mode(ThemeMode::Glassy)>
                <div class="theme-mode-text">
                  <span class="theme-mode-label">"Glassy"</span>
                  <span class="theme-mode-desc">"Translucent blur"</span>
                </div>
              </button>
              <button class=move || if theme_mode.get() == ThemeMode::Flat { "theme-mode-option active" } else { "theme-mode-option" }
                  on:click=move |_| handle_mode(ThemeMode::Flat)>
                <div class="theme-mode-text">
                  <span class="theme-mode-label">"Flat"</span>
                  <span class="theme-mode-desc">"Solid surfaces"</span>
                </div>
              </button>
            </div>
            <div class="theme-section-label">
              <span>"Color Themes"</span>
              <span class="theme-selector-count">{move || filtered.get().len()}</span>
            </div>
            <div class="theme-selector-search">
              <IconSearch size=14 class="w-3.5 h-3.5" />
              <input class="theme-selector-input" node_ref=input_ref type="text"
                  placeholder="Search themes..." prop:value=move || filter.get()
                  on:input=move |e| set_filter.set(event_target_value(&e)) />
            </div>
            <div class="theme-selector-grid">
              {move || {
                if loading.get() {
                  return view! { <div class="theme-selector-loading"><IconLoader2 size=16 class="w-4 h-4 spinning" /><span>"Loading..."</span></div> }.into_any();
                }
                let (items, sel, app) = (filtered.get(), sel_idx.get(), appearance.get());
                if items.is_empty() {
                  return view! { <div class="theme-selector-empty">"No themes found"</div> }.into_any();
                }
                items.into_iter().enumerate().map(|(idx, theme)| {
                  let cls = if idx == sel { "theme-card selected" } else { "theme-card" };
                  let t2 = theme.clone();
                  let c = pick_colors(&theme, app);
                  let (bg, pr, sc, ac, tx) = (c.background.clone(), c.primary.clone(), c.secondary.clone(), c.accent.clone(), c.text.clone());
                  view! {
                    <button class=cls on:click=move |_| handle_apply(t2.clone()) on:mouseenter=move |_| set_sel_idx.set(idx)>
                      <div class="theme-card-preview">
                        <span style=format!("background:{bg};flex:2")></span>
                        <span style=format!("background:{pr};flex:1")></span>
                        <span style=format!("background:{sc};flex:1")></span>
                        <span style=format!("background:{ac};flex:1")></span>
                        <span style=format!("background:{tx};flex:1")></span>
                      </div>
                      <span class="theme-card-name">{theme.name.clone()}</span>
                    </button>
                  }
                }).collect_view().into_any()
              }}
            </div>
            <div class="theme-selector-footer">
              <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Enter"</kbd>" Apply "<kbd>"Esc"</kbd>" Cancel"
            </div>
          </div>
        </div>
    }
}
