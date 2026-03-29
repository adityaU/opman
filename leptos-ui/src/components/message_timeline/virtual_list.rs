//! Virtualization helpers for MessageTimeline.
//! Manages the visible range of groups to render when above VIRTUALIZE_THRESHOLD.

use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

use super::types::{ESTIMATED_ROW_HEIGHT, OVERSCAN, VIRTUALIZE_THRESHOLD};

/// Holds all virtualization-related signals and helpers.
#[derive(Clone, Copy)]
pub struct VirtualState {
    pub virtual_start: ReadSignal<usize>,
    pub set_virtual_start: WriteSignal<usize>,
    pub virtual_end: ReadSignal<usize>,
    pub set_virtual_end: WriteSignal<usize>,
    pub measured_heights: StoredValue<HashMap<usize, f64>>,
}

impl VirtualState {
    pub fn new() -> Self {
        let (virtual_start, set_virtual_start) = signal(0usize);
        let (virtual_end, set_virtual_end) = signal(0usize);
        let measured_heights: StoredValue<HashMap<usize, f64>> = StoredValue::new(HashMap::new());
        Self {
            virtual_start,
            set_virtual_start,
            virtual_end,
            set_virtual_end,
            measured_heights,
        }
    }

    /// Get height of group at index (measured or estimated).
    pub fn get_row_height(&self, index: usize) -> f64 {
        self.measured_heights
            .with_value(|m| m.get(&index).copied().unwrap_or(ESTIMATED_ROW_HEIGHT))
    }

    /// Compute total virtual height for `count` groups.
    pub fn get_total_height(&self, count: usize) -> f64 {
        let mut total = 0.0;
        for i in 0..count {
            total += self.get_row_height(i);
        }
        total
    }

    /// Compute the translateY offset for a given group index.
    pub fn get_offset_for_index(&self, index: usize) -> f64 {
        let mut offset = 0.0;
        for i in 0..index {
            offset += self.get_row_height(i);
        }
        offset
    }

    /// Compute visible range from scroll position. Updates virtual_start / virtual_end.
    pub fn update_range(
        &self,
        groups: Memo<Vec<crate::components::message_turn::MessageGroup>>,
        scroll_container_ref: NodeRef<leptos::html::Div>,
    ) {
        let count = groups.with_untracked(|g| g.len());
        if count < VIRTUALIZE_THRESHOLD {
            self.set_virtual_start.set(0);
            self.set_virtual_end.set(count);
            return;
        }

        let Some(el) = scroll_container_ref.get() else {
            return;
        };
        let el: &web_sys::HtmlElement = &el;
        let scroll_top = el.scroll_top() as f64;
        let viewport_height = el.client_height() as f64;

        let mut offset = 0.0;
        let mut start = 0;
        for i in 0..count {
            let h = self.get_row_height(i);
            if offset + h > scroll_top {
                start = i;
                break;
            }
            offset += h;
            if i == count - 1 {
                start = count;
            }
        }

        let mut end = start;
        let bottom = scroll_top + viewport_height;
        let mut off = offset;
        for i in start..count {
            if off >= bottom {
                end = i;
                break;
            }
            off += self.get_row_height(i);
            if i == count - 1 {
                end = count;
            }
        }

        let os_start = start.saturating_sub(OVERSCAN);
        let os_end = (end + OVERSCAN).min(count);

        self.set_virtual_start.set(os_start);
        self.set_virtual_end.set(os_end);
    }

    /// Measure rendered items and update heights map.
    pub fn measure_items(&self, scroll_container_ref: NodeRef<leptos::html::Div>) {
        let Some(container) = scroll_container_ref.get() else {
            return;
        };
        let el: &web_sys::HtmlElement = &container;
        let Ok(nodes) = el.query_selector_all("[data-index]") else {
            return;
        };

        self.measured_heights.update_value(|map| {
            for i in 0..nodes.length() {
                let Some(node) = nodes.item(i) else { continue };
                let Ok(elem) = node.dyn_into::<web_sys::HtmlElement>() else {
                    continue;
                };
                let Some(idx_str) = elem.get_attribute("data-index") else {
                    continue;
                };
                let Ok(idx) = idx_str.parse::<usize>() else {
                    continue;
                };
                let h = elem.offset_height() as f64;
                if h > 0.0 {
                    let prev = map.get(&idx).copied().unwrap_or(0.0);
                    if (prev - h).abs() > 1.0 {
                        map.insert(idx, h);
                    }
                }
            }
        });
    }
}
