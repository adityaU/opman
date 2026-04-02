//! Interface HTML renderers — avatar, tag-group, toggle, video, audio, separator.
//!
//! Tier 2 blocks for richer interface generation.
//! All functions append raw HTML to `&mut String` for `inner_html`.

use super::html_render::{esc, md_inline, sf, sf_or, svg_icon};

// ── Avatar ──────────────────────────────────────────────────────────

/// Renders avatar(s) — circular image or initials with optional label.
///
/// Data fields:
/// - `avatars` (array, optional): group mode — each has `src`/`name`/`initials`
/// - OR single: `src` (string), `name` (string), `initials` (string)
/// - `size` (string, optional): "sm"|"md"|"lg" (default "md")
pub fn avatar_html(data: &serde_json::Value, out: &mut String) {
    let size = sf(data, "size").unwrap_or_else(|| "md".into());

    if let Some(avatars) = data.get("avatars").and_then(|v| v.as_array()) {
        out.push_str(&format!(
            "<div class=\"a2ui-avatar-group a2ui-avatar-{}\">",
            esc(&size)
        ));
        for a in avatars {
            render_single_avatar(a, out);
        }
        out.push_str("</div>");
        return;
    }
    out.push_str(&format!(
        "<div class=\"a2ui-avatar-single a2ui-avatar-{}\">",
        esc(&size)
    ));
    render_single_avatar(data, out);
    // Optional name label next to avatar
    if let Some(name) = sf(data, "name") {
        out.push_str(&format!(
            "<span class=\"a2ui-avatar-name\">{}</span>",
            esc(&name)
        ));
    }
    out.push_str("</div>");
}

fn render_single_avatar(data: &serde_json::Value, out: &mut String) {
    let src = sf_or(data, "src", "url");
    let name = sf(data, "name").unwrap_or_default();
    let initials = sf(data, "initials").unwrap_or_else(|| make_initials(&name));

    if let Some(ref url) = src {
        out.push_str(&format!(
            "<img class=\"a2ui-avatar\" src=\"{}\" alt=\"{}\" loading=\"lazy\" \
             title=\"{}\" />",
            esc(url),
            esc(&name),
            esc(&name),
        ));
    } else {
        // Generate a hue from name for consistent coloring
        let hue = name_hue(&name);
        out.push_str(&format!(
            "<span class=\"a2ui-avatar a2ui-avatar-initials\" \
             style=\"--avatar-hue:{}\" title=\"{}\">{}</span>",
            hue,
            esc(&name),
            esc(&initials),
        ));
    }
}

fn make_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn name_hue(name: &str) -> u32 {
    let mut hash: u32 = 0;
    for b in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    hash % 360
}

// ── Tag Group ───────────────────────────────────────────────────────

/// Renders a group of interactive/selectable tags.
///
/// Data fields:
/// - `tags` (array, required): each has `label` (string),
///   optional `variant` (string), optional `selected` (bool)
/// - `callback_id` (string, optional): fires callback with selected tag label
pub fn tag_group_html(data: &serde_json::Value, out: &mut String) {
    let tags = match data.get("tags").and_then(|v| v.as_array()) {
        Some(t) => t,
        None => return,
    };
    let callback_id = sf(data, "callback_id");
    let has_cb = callback_id.is_some();

    out.push_str("<div class=\"a2ui-tag-group\">");
    for tag in tags {
        let label = tag.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let variant = tag
            .get("variant")
            .and_then(|v| v.as_str())
            .unwrap_or("neutral");
        let selected = tag
            .get("selected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let sel_cls = if selected { " a2ui-tag-selected" } else { "" };

        if has_cb {
            out.push_str(&format!(
                "<button class=\"a2ui-tag a2ui-tag-{}{}\" \
                 data-a2ui-callback=\"{}\" data-a2ui-tag-value=\"{}\">#{}</button>",
                esc(variant),
                sel_cls,
                esc(callback_id.as_deref().unwrap_or("")),
                esc(label),
                esc(label),
            ));
        } else {
            out.push_str(&format!(
                "<span class=\"a2ui-tag a2ui-tag-{}{}\">{}</span>",
                esc(variant),
                sel_cls,
                esc(label),
            ));
        }
    }
    out.push_str("</div>");
}

// ── Toggle ──────────────────────────────────────────────────────────

/// Renders an on/off toggle switch.
///
/// Data fields:
/// - `label` (string, required): toggle label
/// - `checked` / `value` (bool, default false): initial state
/// - `callback_id` (string, optional): fires callback with {value: true/false}
/// - `description` (string, optional): help text
pub fn toggle_html(data: &serde_json::Value, out: &mut String) {
    let label = sf(data, "label").unwrap_or_default();
    let checked = data
        .get("checked")
        .or_else(|| data.get("value"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let callback_id = sf(data, "callback_id");
    let desc = sf(data, "description");

    let uid = toggle_uid();
    let checked_attr = if checked { " checked" } else { "" };

    out.push_str("<div class=\"a2ui-toggle\">");
    out.push_str(&format!(
        "<label class=\"a2ui-toggle-label\" for=\"{uid}\">{}</label>",
        esc(&label)
    ));
    out.push_str(&format!(
        "<div class=\"a2ui-toggle-switch\">\
         <input type=\"checkbox\" id=\"{uid}\" class=\"a2ui-toggle-input\"{checked_attr}",
    ));
    if let Some(ref cb) = callback_id {
        out.push_str(&format!(" data-a2ui-callback=\"{}\"", esc(cb)));
    }
    out.push_str(" />");
    out.push_str(&format!(
        "<label for=\"{uid}\" class=\"a2ui-toggle-track\"></label></div>"
    ));
    if let Some(d) = desc {
        out.push_str(&format!(
            "<span class=\"a2ui-toggle-desc\">{}</span>",
            md_inline(&d)
        ));
    }
    out.push_str("</div>");
}

fn toggle_uid() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CTR: AtomicU32 = AtomicU32::new(0);
    format!("a2tg{}", CTR.fetch_add(1, Ordering::Relaxed))
}

// ── Video ───────────────────────────────────────────────────────────

/// Renders an embedded video player.
///
/// Data fields:
/// - `url` / `src` (string, required): video URL
/// - `poster` (string, optional): poster image URL
/// - `title` / `caption` (string, optional): caption text
/// - `width` (string, optional): CSS width
/// - `height` (string, optional): CSS height
pub fn video_html(data: &serde_json::Value, out: &mut String) {
    let url = match sf_or(data, "url", "src") {
        Some(u) => u,
        None => {
            out.push_str("<div class=\"a2ui-unknown\">Video: missing url</div>");
            return;
        }
    };
    let poster = sf(data, "poster");
    let caption = sf_or(data, "title", "caption");
    let width = sf(data, "width");
    let height = sf(data, "height");

    out.push_str("<figure class=\"a2ui-video\">");
    out.push_str("<video controls preload=\"metadata\" class=\"a2ui-video-el\"");
    if let Some(ref p) = poster {
        out.push_str(&format!(" poster=\"{}\"", esc(p)));
    }
    let mut style = String::new();
    if let Some(ref w) = width {
        style.push_str(&format!("max-width:{};", esc(w)));
    }
    if let Some(ref h) = height {
        style.push_str(&format!("max-height:{};", esc(h)));
    }
    if !style.is_empty() {
        out.push_str(&format!(" style=\"{style}\""));
    }
    out.push_str(&format!(
        "><source src=\"{}\" />Your browser does not support video.</video>",
        esc(&url)
    ));
    if let Some(cap) = caption {
        out.push_str(&format!(
            "<figcaption class=\"a2ui-video-caption\">{}</figcaption>",
            md_inline(&cap)
        ));
    }
    out.push_str("</figure>");
}

// ── Audio ───────────────────────────────────────────────────────────

/// Renders an embedded audio player.
///
/// Data fields:
/// - `url` / `src` (string, required): audio URL
/// - `title` / `label` (string, optional): label above player
pub fn audio_html(data: &serde_json::Value, out: &mut String) {
    let url = match sf_or(data, "url", "src") {
        Some(u) => u,
        None => {
            out.push_str("<div class=\"a2ui-unknown\">Audio: missing url</div>");
            return;
        }
    };
    let title = sf_or(data, "title", "label");

    out.push_str("<div class=\"a2ui-audio\">");
    if let Some(ref t) = title {
        out.push_str(&format!("<div class=\"a2ui-audio-title\">{}</div>", esc(t)));
    }
    out.push_str(&format!(
        "<audio controls preload=\"metadata\" class=\"a2ui-audio-el\">\
         <source src=\"{}\" />Your browser does not support audio.</audio>",
        esc(&url)
    ));
    out.push_str("</div>");
}

// ── Separator ───────────────────────────────────────────────────────

/// Renders a decorative section separator with optional icon/emoji.
///
/// Data fields:
/// - `icon` / `emoji` (string, optional): center decoration
/// - `label` (string, optional): center text
/// - `style` (string, optional): "solid"|"dashed"|"dotted" (default "solid")
pub fn separator_html(data: &serde_json::Value, out: &mut String) {
    let icon = sf_or(data, "icon", "emoji");
    let label = sf(data, "label");
    let line_style = sf(data, "style").unwrap_or_else(|| "solid".into());

    out.push_str(&format!(
        "<div class=\"a2ui-separator a2ui-sep-{}\">",
        esc(&line_style)
    ));
    out.push_str("<hr class=\"a2ui-sep-line\" />");
    if let Some(ref ic) = icon {
        out.push_str(&format!("<span class=\"a2ui-sep-icon\">{}</span>", esc(ic)));
    } else if let Some(ref l) = label {
        out.push_str(&format!("<span class=\"a2ui-sep-label\">{}</span>", esc(l)));
    }
    out.push_str("</div>");
}
