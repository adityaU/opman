//! Apply theme colors and appearance (light/dark/system) to CSS custom properties.

use crate::types::api::ThemeColors;
use wasm_bindgen::JsCast;

/// Apply the 15 core theme colors as CSS custom properties on `<html>`.
/// Also updates `<meta name="theme-color">`, `<meta name="color-scheme">`,
/// paints all background layers, and syncs PWA icons.
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

    // Paint html — size + background so Android Chrome samples correct color
    let _ = style.set_property("width", "100%");
    let _ = style.set_property("height", "100%");
    let _ = style.set_property("overflow", "hidden");
    let _ = style.set_property("background", &colors.background);
    let _ = style.set_property("background-color", &colors.background);

    // Paint body — must also fill viewport so no transparent gap
    if let Some(body) = document.body() {
        let bs = body.style();
        let _ = bs.set_property("width", "100%");
        let _ = bs.set_property("height", "100%");
        let _ = bs.set_property("overflow", "hidden");
        let _ = bs.set_property("margin", "0");
        let _ = bs.set_property("background", &colors.background);
        let _ = bs.set_property("background-color", &colors.background);
    }

    // Paint #leptos-root — fill viewport with overflow hidden
    if let Ok(Some(app_root)) = document.query_selector("#leptos-root") {
        if let Some(el) = app_root.dyn_ref::<web_sys::HtmlElement>() {
            let rs = el.style();
            let _ = rs.set_property("width", "100%");
            let _ = rs.set_property("height", "100%");
            let _ = rs.set_property("overflow", "hidden");
            let _ = rs.set_property("position", "relative");
            let _ = rs.set_property("background", &colors.background);
            let _ = rs.set_property("background-color", &colors.background);
        }
    }

    // Update meta theme-color + color-scheme
    sync_meta_theme_color(&colors.background);
    sync_color_scheme(&colors.background, &style);

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

// ── Appearance (light / dark / system) ──────────────────────────────

/// Read the persisted appearance setting from localStorage.
/// Returns `"system"`, `"light"`, or `"dark"`.
pub fn get_appearance() -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("opman-appearance").ok())
        .flatten()
        .unwrap_or_else(|| "dark".to_string())
}

/// Persist the appearance setting and toggle `html.light-theme` class.
pub fn set_appearance(appearance: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("opman-appearance", appearance);
    }
    apply_appearance_class(appearance);
}

/// Resolve appearance to the effective mode: `"dark"` or `"light"`.
pub fn resolve_appearance(appearance: &str) -> &'static str {
    match appearance {
        "light" => "light",
        "dark" => "dark",
        _ => {
            // "system" — check OS preference
            if system_prefers_dark() {
                "dark"
            } else {
                "light"
            }
        }
    }
}

/// Check whether the OS prefers dark mode via `matchMedia`.
pub fn system_prefers_dark() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok())
        .flatten()
        .map(|mq| mq.matches())
        .unwrap_or(true) // default to dark if unknown
}

/// Apply the `light-theme` CSS class based on the effective mode.
fn apply_appearance_class(appearance: &str) {
    let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    else {
        return;
    };
    let effective = resolve_appearance(appearance);
    let cl = root.class_list();
    if effective == "light" {
        let _ = cl.add_1("light-theme");
    } else {
        let _ = cl.remove_1("light-theme");
    }
    // Also set data-appearance for CSS selectors
    let _ = root.set_attribute("data-appearance", effective);
}

/// Initialize appearance from localStorage on startup.
pub fn init_appearance() {
    let appearance = get_appearance();
    apply_appearance_class(&appearance);
}

/// Update all `<meta name="theme-color">` tags, creating them if missing.
fn sync_meta_theme_color(bg: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    let Ok(nodes) = document.query_selector_all("meta[name=\"theme-color\"]") else {
        return;
    };

    if nodes.length() > 0 {
        for i in 0..nodes.length() {
            if let Some(node) = nodes.item(i) {
                if let Some(el) = node.dyn_ref::<web_sys::Element>() {
                    let _ = el.set_attribute("content", bg);
                }
            }
        }
        return;
    }

    // No meta theme-color tags exist — create the three variants
    let head = match document.head() {
        Some(h) => h,
        None => return,
    };
    for media in &[
        "(prefers-color-scheme: dark)",
        "(prefers-color-scheme: light)",
        "",
    ] {
        let Ok(meta) = document.create_element("meta") else {
            continue;
        };
        let _ = meta.set_attribute("name", "theme-color");
        let _ = meta.set_attribute("content", bg);
        if !media.is_empty() {
            let _ = meta.set_attribute("media", media);
        }
        let _ = head.append_child(&meta);
    }
}

/// Determine if a hex color is dark using sRGB relative luminance.
/// Returns `true` when luminance < 0.2 (perceived as dark).
fn is_dark_color(hex: &str) -> bool {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return true; // default to dark for malformed input
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;

    // sRGB to linear
    let to_linear = |c: f64| -> f64 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };

    let luminance = 0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b);
    luminance < 0.2
}

/// Sync the `<meta name="color-scheme">` tag and CSS property.
/// Android Chrome derives the navigation bar dark/light mode from this.
fn sync_color_scheme(bg: &str, html_style: &web_sys::CssStyleDeclaration) {
    let scheme = if is_dark_color(bg) { "dark" } else { "light" };

    // Update CSS property on <html>
    let _ = html_style.set_property("color-scheme", scheme);

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    // Update existing <meta name="color-scheme"> or create one
    if let Ok(Some(meta)) = document.query_selector("meta[name=\"color-scheme\"]") {
        let _ = meta.set_attribute("content", scheme);
    } else if let Some(head) = document.head() {
        if let Ok(meta) = document.create_element("meta") {
            let _ = meta.set_attribute("name", "color-scheme");
            let _ = meta.set_attribute("content", scheme);
            let _ = head.append_child(&meta);
        }
    }
}
