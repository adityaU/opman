//! Apply theme colors to CSS custom properties on the document element.

use crate::types::api::ThemeColors;
use wasm_bindgen::JsCast;

/// Apply the 15 core theme colors as CSS custom properties on `<html>`.
/// Also updates `<meta name="theme-color">`, background colors, and `color-scheme`.
pub fn apply_theme_to_css(colors: &ThemeColors) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };
    let html = root.unchecked_ref::<web_sys::HtmlElement>();
    let style = html.style();

    // Set all 15 core CSS variables
    let vars = [
        ("--color-primary", &colors.primary),
        ("--color-secondary", &colors.secondary),
        ("--color-accent", &colors.accent),
        ("--color-bg", &colors.background),
        ("--color-bg-panel", &colors.background_panel),
        ("--color-bg-element", &colors.background_element),
        ("--color-text", &colors.text),
        ("--color-text-muted", &colors.text_muted),
        ("--color-border", &colors.border),
        ("--color-border-active", &colors.border_active),
        ("--color-border-subtle", &colors.border_subtle),
        ("--color-error", &colors.error),
        ("--color-warning", &colors.warning),
        ("--color-success", &colors.success),
        ("--color-info", &colors.info),
    ];

    for (prop, value) in &vars {
        let _ = style.set_property(prop, value);
    }

    // Update html/body background
    let _ = style.set_property("background", &colors.background);

    // Update meta theme-color
    sync_meta_theme_color(&colors.background);

    // Sync favicon, PWA icons, and service worker to match theme
    super::icons::update_favicon(&colors.primary, &colors.background);
    super::icons::update_pwa_icons(&colors.primary, &colors.background);
    super::icons::notify_service_worker(&colors.primary, &colors.background);
}

/// Get the current theme mode from localStorage.
pub fn get_theme_mode() -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("opman-theme-mode").ok())
        .flatten()
        .unwrap_or_else(|| "glassy".to_string())
}

/// Set the theme mode (glassy/flat) and apply the corresponding CSS class.
pub fn set_theme_mode(mode: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };

    // Persist to localStorage
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item("opman-theme-mode", mode);
    }

    // Toggle flat-theme class
    let class_list = root.class_list();
    if mode == "flat" {
        let _ = class_list.add_1("flat-theme");
    } else {
        let _ = class_list.remove_1("flat-theme");
    }
}

/// Apply theme mode from localStorage on startup.
pub fn init_theme_mode() {
    let mode = get_theme_mode();
    set_theme_mode(&mode);
}

/// Update the `<meta name="theme-color">` tag.
fn sync_meta_theme_color(bg: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    if let Ok(nodes) = document.query_selector_all("meta[name=\"theme-color\"]") {
        for i in 0..nodes.length() {
            if let Some(node) = nodes.item(i) {
                if let Some(el) = node.dyn_ref::<web_sys::Element>() {
                    let _ = el.set_attribute("content", bg);
                }
            }
        }
    }
}
