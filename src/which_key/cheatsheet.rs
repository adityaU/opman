use crossterm::event::KeyModifiers;

use crate::vim_mode::VimMode;

use super::key_combo::format_key_label;
use super::types::RuntimeKeyBinding;

fn collect_bindings(
    bindings: &[RuntimeKeyBinding],
    mode: VimMode,
    prefix_str: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for b in bindings {
        if !b.active_in(mode) {
            continue;
        }
        let key_label = if prefix_str.is_empty() {
            format_key_label(&b.key)
        } else {
            format!("{}{}", prefix_str, format_key_label(&b.key))
        };
        if b.children.is_empty() {
            result.push((key_label, b.label.clone()));
        } else {
            let child_prefix = format!("{}+ ", key_label);
            result.extend(collect_bindings(&b.children, mode, &child_prefix));
        }
    }
    result
}

/// Generate cheatsheet sections from the runtime keymap.
pub fn generate_cheatsheet_sections(
    keymap: &[RuntimeKeyBinding],
) -> Vec<(String, Vec<(String, String)>)> {
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();

    // All Modes — bindings active everywhere (modes is empty, no context, no children)
    let all_mode: Vec<_> = keymap
        .iter()
        .filter(|b| b.modes.is_empty() && b.context.is_none() && b.children.is_empty())
        .map(|b| (format_key_label(&b.key), b.label.clone()))
        .collect();
    if !all_mode.is_empty() {
        sections.push(("All Modes".to_string(), all_mode));
    }

    // Normal Mode — non-modifier, non-prefix keys
    let mut normal = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || !b.active_in(VimMode::Normal) || b.modes.is_empty() {
            continue;
        }
        if b.key.modifiers.is_empty() && b.children.is_empty() {
            normal.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !normal.is_empty() {
        sections.push(("Normal Mode".to_string(), normal));
    }

    // Ctrl+ (Normal) — modifier keys in normal mode
    let mut ctrl = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || !b.active_in(VimMode::Normal) || b.modes.is_empty() {
            continue;
        }
        if b.children.is_empty() && b.key.modifiers.contains(KeyModifiers::CONTROL) {
            ctrl.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !ctrl.is_empty() {
        sections.push(("Ctrl+ (Normal)".to_string(), ctrl));
    }

    // Leader+ (Normal) — find the leader prefix binding and collect its children
    if let Some(leader) = keymap
        .iter()
        .find(|b| !b.children.is_empty() && b.context.is_none() && b.active_in(VimMode::Normal))
    {
        let space = collect_bindings(&leader.children, VimMode::Normal, "");
        if !space.is_empty() {
            let leader_label = format_key_label(&leader.key);
            sections.push((format!("{}+ (Normal)", leader_label), space));
        }
    }

    let mut insert = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || b.modes.is_empty() {
            continue;
        }
        if b.active_in(VimMode::Insert) && b.children.is_empty() {
            insert.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !insert.is_empty() {
        sections.push(("Insert Mode".to_string(), insert));
    }

    let resize: Vec<_> = keymap
        .iter()
        .filter(|b| b.context.is_none() && b.modes.contains(&VimMode::Resize))
        .map(|b| (format_key_label(&b.key), b.label.clone()))
        .collect();
    if !resize.is_empty() {
        sections.push(("Resize Mode".to_string(), resize));
    }

    // Context-grouped sections (Sidebar, Insert Mode, etc.)
    let mut context_map: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for b in keymap.iter() {
        if let Some(ref ctx) = b.context {
            let key_label = format_key_label(&b.key);
            if let Some(section) = context_map.iter_mut().find(|(name, _)| name == ctx) {
                section.1.push((key_label, b.label.clone()));
            } else {
                context_map.push((ctx.clone(), vec![(key_label, b.label.clone())]));
            }
        }
    }
    sections.extend(context_map);

    sections
}
