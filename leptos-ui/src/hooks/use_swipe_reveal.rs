//! Swipe-left-to-reveal hook for mobile list row actions.
//!
//! Tracks touch gestures on a container element and exposes reactive
//! signals that drive the CSS `translateX` of the content layer.
//!
//! Usage:
//! ```rust,ignore
//! let swipe = use_swipe_reveal(SwipeConfig { actions_width: 152.0 });
//! view! {
//!     <div
//!         class=move || swipe.container_class()
//!         on:touchstart=swipe.on_touch_start()
//!         on:touchmove=swipe.on_touch_move()
//!         on:touchend=swipe.on_touch_end()
//!     >
//!         <div class="swipe-row-actions">/* action buttons */</div>
//!         <div class="swipe-row-content" style=move || swipe.content_style()>
//!             /* normal row content */
//!         </div>
//!     </div>
//! }
//! ```

use leptos::prelude::*;

/// Configuration for the swipe-reveal hook.
pub struct SwipeConfig {
    /// Total width (px) of the revealed action tray.
    pub actions_width: f64,
}

/// State returned by [`use_swipe_reveal`].
///
/// Uses `StoredValue` for internal touch tracking (Send-safe on wasm)
/// and Leptos signals for reactive state.
#[derive(Clone, Copy)]
pub struct SwipeState {
    /// Current translateX offset signal (always <= 0).
    offset: ReadSignal<f64>,
    set_offset: WriteSignal<f64>,
    /// Whether the user is actively swiping (finger down + moved).
    swiping: ReadSignal<bool>,
    set_swiping: WriteSignal<bool>,
    /// Whether the tray is latched open.
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    /// Internal touch tracking state (StoredValue is Send-safe).
    start_x: StoredValue<f64>,
    start_y: StoredValue<f64>,
    start_offset: StoredValue<f64>,
    /// None = undecided, Some(true) = horizontal, Some(false) = vertical.
    is_horizontal: StoredValue<Option<bool>>,
    /// Max negative offset (= -actions_width).
    max_offset: f64,
}

impl SwipeState {
    /// CSS class additions for the `.swipe-row` container.
    pub fn container_class(&self) -> String {
        let mut cls = String::from("swipe-row");
        if self.swiping.get() {
            cls.push_str(" swiping");
        }
        if self.open.get() {
            cls.push_str(" swipe-open");
        }
        cls
    }

    /// Inline style for the `.swipe-row-content` layer.
    pub fn content_style(&self) -> String {
        let off = self.offset.get();
        if off == 0.0 {
            return String::new();
        }
        format!("transform:translateX({}px)", off)
    }

    /// Close the revealed tray programmatically.
    pub fn close(&self) {
        self.set_offset.set(0.0);
        self.set_open.set(false);
        self.set_swiping.set(false);
    }

    /// Returns a closure suitable for `on:touchstart`.
    pub fn on_touch_start(&self) -> impl Fn(web_sys::TouchEvent) + 'static {
        let start_x = self.start_x;
        let start_y = self.start_y;
        let start_offset = self.start_offset;
        let is_horizontal = self.is_horizontal;
        let offset = self.offset;
        move |ev: web_sys::TouchEvent| {
            let Some(touch) = ev.touches().get(0) else {
                return;
            };
            start_x.set_value(touch.client_x() as f64);
            start_y.set_value(touch.client_y() as f64);
            start_offset.set_value(offset.get_untracked());
            is_horizontal.set_value(None);
        }
    }

    /// Returns a closure suitable for `on:touchmove`.
    pub fn on_touch_move(&self) -> impl Fn(web_sys::TouchEvent) + 'static {
        let start_x = self.start_x;
        let start_y = self.start_y;
        let start_offset = self.start_offset;
        let is_horizontal = self.is_horizontal;
        let set_offset = self.set_offset;
        let set_swiping = self.set_swiping;
        let max_offset = self.max_offset;
        move |ev: web_sys::TouchEvent| {
            let Some(touch) = ev.touches().get(0) else {
                return;
            };
            let dx = touch.client_x() as f64 - start_x.get_value();
            let dy = touch.client_y() as f64 - start_y.get_value();

            // Determine gesture direction on first significant move
            if is_horizontal.get_value().is_none() {
                let adx = dx.abs();
                let ady = dy.abs();
                if adx < 5.0 && ady < 5.0 {
                    return; // too small to decide
                }
                is_horizontal.set_value(Some(adx > ady));
            }

            // If vertical scroll, bail out
            if is_horizontal.get_value() != Some(true) {
                return;
            }

            // Prevent vertical scroll while swiping horizontally
            ev.prevent_default();
            set_swiping.set(true);

            let raw = start_offset.get_value() + dx;
            // Clamp: no rightward past 0, no leftward past max
            let clamped = raw.max(max_offset).min(0.0);
            set_offset.set(clamped);
        }
    }

    /// Returns a closure suitable for `on:touchend` / `on:touchcancel`.
    pub fn on_touch_end(&self) -> impl Fn(web_sys::TouchEvent) + 'static {
        let set_offset = self.set_offset;
        let set_swiping = self.set_swiping;
        let set_open = self.set_open;
        let offset = self.offset;
        let max_offset = self.max_offset;
        move |_ev: web_sys::TouchEvent| {
            set_swiping.set(false);
            let cur = offset.get_untracked();
            // Snap threshold: if dragged past 40% of action width, open; else close
            let threshold = max_offset * 0.4;
            if cur < threshold {
                set_offset.set(max_offset);
                set_open.set(true);
            } else {
                set_offset.set(0.0);
                set_open.set(false);
            }
        }
    }
}

/// Create swipe-reveal state for a single list row.
///
/// Call once per row. Returns [`SwipeState`] with reactive signals
/// and touch event handler closures.
pub fn use_swipe_reveal(config: SwipeConfig) -> SwipeState {
    let (offset, set_offset) = signal(0.0_f64);
    let (swiping, set_swiping) = signal(false);
    let (open, set_open) = signal(false);

    SwipeState {
        offset,
        set_offset,
        swiping,
        set_swiping,
        open,
        set_open,
        start_x: StoredValue::new(0.0),
        start_y: StoredValue::new(0.0),
        start_offset: StoredValue::new(0.0),
        is_horizontal: StoredValue::new(None),
        max_offset: -config.actions_width.abs(),
    }
}
