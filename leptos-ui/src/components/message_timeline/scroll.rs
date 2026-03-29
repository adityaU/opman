//! Scroll handling for MessageTimeline: auto-scroll, direction detection,
//! jump-to-bottom, load-older, and search-match scrolling.

use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

use super::types::request_animation_frame;

/// All signals and stored values the scroll handler needs.
/// Built once in the parent and passed by value (Copy) to closures.
#[derive(Clone, Copy)]
pub struct ScrollState {
    pub scroll_container_ref: NodeRef<leptos::html::Div>,
    pub should_auto_scroll: ReadSignal<bool>,
    pub set_should_auto_scroll: WriteSignal<bool>,
    pub show_jump_to_bottom: ReadSignal<bool>,
    pub set_show_jump_to_bottom: WriteSignal<bool>,
    pub last_scroll_top: ReadSignal<i32>,
    pub set_last_scroll_top: WriteSignal<i32>,
    pub cumulative_delta: ReadSignal<i32>,
    pub set_cumulative_delta: WriteSignal<i32>,
    pub programmatic_scroll: ReadSignal<bool>,
    pub set_programmatic_scroll: WriteSignal<bool>,
    pub scroll_raf_pending: ReadSignal<bool>,
    pub set_scroll_raf_pending: WriteSignal<bool>,
}

const SCROLL_DIRECTION_THRESHOLD: i32 = 20;

impl ScrollState {
    pub fn new() -> Self {
        let (should_auto_scroll, set_should_auto_scroll) = signal(true);
        let (show_jump_to_bottom, set_show_jump_to_bottom) = signal(false);
        let (last_scroll_top, set_last_scroll_top) = signal(0i32);
        let (cumulative_delta, set_cumulative_delta) = signal(0i32);
        let (programmatic_scroll, set_programmatic_scroll) = signal(false);
        let (scroll_raf_pending, set_scroll_raf_pending) = signal(false);
        Self {
            scroll_container_ref: NodeRef::new(),
            should_auto_scroll,
            set_should_auto_scroll,
            show_jump_to_bottom,
            set_show_jump_to_bottom,
            last_scroll_top,
            set_last_scroll_top,
            cumulative_delta,
            set_cumulative_delta,
            programmatic_scroll,
            set_programmatic_scroll,
            scroll_raf_pending,
            set_scroll_raf_pending,
        }
    }

    /// Reset scroll state on session switch.
    pub fn reset(&self) {
        self.set_should_auto_scroll.set(true);
        self.set_show_jump_to_bottom.set(false);
        self.set_last_scroll_top.set(0);
        self.set_cumulative_delta.set(0);
    }

    /// Scroll container to bottom (programmatic).
    pub fn scroll_to_bottom(&self) {
        if let Some(el) = self.scroll_container_ref.get() {
            self.set_programmatic_scroll.set(true);
            let el: &web_sys::HtmlElement = &el;
            el.set_scroll_top(el.scroll_height());
        }
    }

    /// Build the on_scroll event handler closure.
    pub fn build_on_scroll(
        &self,
        use_virtual: Memo<bool>,
        has_older: ReadSignal<bool>,
        is_loading_older: ReadSignal<bool>,
        stored_scroll_dir: StoredValue<Option<Callback<String>>>,
        stored_load_older: StoredValue<Option<Callback<()>>>,
        sse: crate::hooks::use_sse_state::SseState,
        update_virtual_range: impl Fn() + Copy + 'static,
    ) -> impl Fn(web_sys::Event) + 'static {
        let ss = *self;
        move |_: web_sys::Event| {
            let Some(el) = ss.scroll_container_ref.get() else { return };
            let el: &web_sys::HtmlElement = &el;
            let current_scroll_top = el.scroll_top();
            let scroll_height = el.scroll_height();
            let client_height = el.client_height();
            let distance_from_bottom = scroll_height - current_scroll_top - client_height;

            let delta = current_scroll_top - ss.last_scroll_top.get_untracked();
            ss.set_last_scroll_top.set(current_scroll_top);

            let near_bottom = distance_from_bottom < 100;

            // Throttled virtualization range update via rAF
            if use_virtual.get_untracked() && !ss.scroll_raf_pending.get_untracked() {
                ss.set_scroll_raf_pending.set(true);
                request_animation_frame(move || {
                    update_virtual_range();
                    ss.set_scroll_raf_pending.set(false);
                });
            }

            // Programmatic scrolls — update UI state only
            if ss.programmatic_scroll.get_untracked() {
                ss.set_programmatic_scroll.set(false);
                ss.set_cumulative_delta.set(0);
                ss.set_show_jump_to_bottom.set(
                    !near_bottom && scroll_height > client_height + 200,
                );
                return;
            }

            if delta < -5 {
                ss.set_should_auto_scroll.set(false);
            }
            if near_bottom {
                ss.set_should_auto_scroll.set(true);
            }
            ss.set_show_jump_to_bottom.set(
                !ss.should_auto_scroll.get_untracked() && !near_bottom,
            );

            // Direction detection
            stored_scroll_dir.with_value(|dir_opt| {
                let Some(ref dir_cb) = dir_opt else { return };
                let cum = ss.cumulative_delta.get_untracked();
                if (cum > 0 && delta < 0) || (cum < 0 && delta > 0) {
                    ss.set_cumulative_delta.set(0);
                }
                let new_cum = ss.cumulative_delta.get_untracked() + delta;
                ss.set_cumulative_delta.set(new_cum);
                if new_cum.abs() >= SCROLL_DIRECTION_THRESHOLD {
                    let direction = if new_cum < 0 { "up" } else { "down" };
                    ss.set_cumulative_delta.set(0);
                    dir_cb.run(direction.to_string());
                }
            });

            // Load older messages
            if current_scroll_top < 200 && has_older.get_untracked() && !is_loading_older.get_untracked() {
                let handled = stored_load_older.with_value(|lo_opt| {
                    if let Some(ref cb) = lo_opt {
                        cb.run(());
                        true
                    } else {
                        false
                    }
                });
                if !handled {
                    let prev_scroll_height = scroll_height;
                    sse.load_older_messages();
                    let el_clone = el.clone();
                    leptos::task::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(50).await;
                        let new_scroll_height = el_clone.scroll_height();
                        let height_diff = new_scroll_height - prev_scroll_height;
                        if height_diff > 0 {
                            el_clone.set_scroll_top(current_scroll_top + height_diff);
                        }
                    });
                }
            }
        }
    }
}

/// Set up the search-match scroll effect.
pub fn setup_search_scroll_effect(
    active_search_match_id: Option<ReadSignal<Option<String>>>,
    message_id_to_group_key: Memo<HashMap<String, String>>,
    use_virtual: Memo<bool>,
    groups: Memo<Vec<crate::components::message_turn::MessageGroup>>,
    ss: ScrollState,
    get_offset_for_index: impl Fn(usize) -> f64 + Copy + 'static,
    update_virtual_range: impl Fn() + Copy + 'static,
) {
    let Some(active_match_signal) = active_search_match_id else { return };
    let container_ref = ss.scroll_container_ref;

    Effect::new(move |_| {
        let active_id = match active_match_signal.get() {
            Some(id) if !id.is_empty() => id,
            _ => return,
        };
        let map = message_id_to_group_key.get();
        let Some(group_key) = map.get(&active_id).cloned() else { return };

        if use_virtual.get_untracked() {
            let gs = groups.get_untracked();
            if let Some(group_idx) = gs.iter().position(|g| g.key == group_key) {
                if let Some(container) = container_ref.get() {
                    let offset = get_offset_for_index(group_idx);
                    let el: &web_sys::HtmlElement = &container;
                    let viewport_h = el.client_height() as f64;
                    let target_scroll = (offset - viewport_h / 2.0).max(0.0);
                    ss.set_programmatic_scroll.set(true);
                    el.set_scroll_top(target_scroll as i32);
                    update_virtual_range();
                }
            }
        }

        let container_ref2 = container_ref;
        let gk = group_key;
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(16).await;
            let Some(container) = container_ref2.get() else { return };
            let el: &web_sys::HtmlElement = &container;
            let selector = format!("[data-group-key=\"{}\"]", gk.replace('"', "\\\""));
            if let Ok(Some(target)) = el.query_selector(&selector) {
                let opts = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&opts, &"behavior".into(), &"smooth".into());
                let _ = js_sys::Reflect::set(&opts, &"block".into(), &"center".into());
                if let Ok(func) = js_sys::Reflect::get(&target, &"scrollIntoView".into()) {
                    if let Ok(func) = func.dyn_into::<js_sys::Function>() {
                        let _ = func.call1(&target, &opts);
                    }
                }
            }
        });
    });
}
