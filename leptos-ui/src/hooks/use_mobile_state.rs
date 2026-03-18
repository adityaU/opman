//! Mobile/tablet state management.
//! Extends React `useMobileState.ts` with tablet breakpoint.
//! Phone: <= 768px | Tablet: 769–1400px | Desktop: > 1400px
//!
//! **Layout policy**: phone + tablet use the floating dock/FAB/compose/input-hide UX.
//! Only phones use fullscreen panel sheets (git/editor/terminal overlays).
//! Tablets keep desktop split panels + desktop modals but get mobile dock + input-hide.
//! Desktop uses the side-panel / terminal-tray / resizable-handle layout only.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Device mode — phone, tablet, or desktop.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DeviceMode {
    Phone,
    Tablet,
    Desktop,
}

impl DeviceMode {
    pub fn is_phone(self) -> bool {
        self == Self::Phone
    }
    pub fn is_tablet(self) -> bool {
        self == Self::Tablet
    }
    pub fn is_desktop(self) -> bool {
        self == Self::Desktop
    }
    /// True for phone and tablet — they use the floating dock/FAB/compose UX.
    /// Desktop uses the status-bar layout only.
    pub fn uses_dock(self) -> bool {
        !self.is_desktop()
    }
    /// True for phone only — only phones use fullscreen panel sheets
    /// (git/editor/terminal overlays). Tablet + desktop use side-panel split.
    pub fn uses_panel_sheets(self) -> bool {
        self.is_phone()
    }
    /// True for phone or tablet (touch-primary devices).
    pub fn is_touch_device(self) -> bool {
        !self.is_desktop()
    }
}

const PHONE_MAX: f64 = 768.0;
const TABLET_MAX: f64 = 1400.0;

/// Detect whether the device is a touch-capable tablet (iPad, Android tablet).
/// iPads in landscape report widths > 1400 and a desktop-class UA, so width
/// alone is unreliable. We combine touch capability + platform hints.
///
/// Uses `any-pointer: coarse` as well as `pointer: coarse` so that iPads with
/// an attached keyboard/trackpad (which promote `pointer` to `fine`) are still
/// recognised. Falls back to `maxTouchPoints` for environments where media
/// queries are unavailable.
fn is_touch_tablet() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };

    let match_media = |q: &str| -> bool {
        window
            .match_media(q)
            .ok()
            .flatten()
            .map_or(false, |m| m.matches())
    };

    // Primary pointer is coarse (no external keyboard/trackpad attached)
    let has_coarse = match_media("(pointer: coarse)");
    // *Any* pointer is coarse — true even when keyboard/trackpad is primary
    let any_coarse = match_media("(any-pointer: coarse)");
    // Fallback: device exposes touch points (covers edge cases)
    let has_touch_points = window.navigator().max_touch_points() > 0;

    if !has_coarse && !any_coarse && !has_touch_points {
        return false;
    }

    // Check navigator platform / userAgent for tablet signatures
    let nav = window.navigator();
    let platform = nav.platform().unwrap_or_default().to_lowercase();
    let ua = nav.user_agent().unwrap_or_default().to_lowercase();

    // iPad: modern iPadOS reports "MacIntel" platform with touch capability.
    // Older iPads expose "ipad" in the platform string.
    if platform.contains("ipad")
        || (platform.contains("mac") && (has_coarse || any_coarse || has_touch_points))
    {
        return true;
    }

    // Android tablet: has "android" in UA but NOT "mobile"
    if ua.contains("android") && !ua.contains("mobile") {
        return true;
    }

    false
}

fn detect_device_mode() -> DeviceMode {
    let width = web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);

    // Phone — always width-based
    if width <= PHONE_MAX {
        return DeviceMode::Phone;
    }

    // Tablet — either within tablet width range, or a touch-tablet in fullscreen
    if width <= TABLET_MAX || is_touch_tablet() {
        return DeviceMode::Tablet;
    }

    DeviceMode::Desktop
}

/// Which mobile panel is active.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MobilePanel {
    Opencode,
    Git,
    Editor,
    Terminal,
}

impl MobilePanel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::Git => "git",
            Self::Editor => "editor",
            Self::Terminal => "terminal",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Opencode => "Chat",
            Self::Git => "Git",
            Self::Editor => "Editor",
            Self::Terminal => "Terminal",
        }
    }
}

/// Mobile/tablet-specific UI state.
#[derive(Clone, Copy)]
pub struct MobileState {
    pub sidebar_open: ReadSignal<bool>,
    pub set_sidebar_open: WriteSignal<bool>,
    pub active_panel: ReadSignal<Option<MobilePanel>>,
    set_active_panel: WriteSignal<Option<MobilePanel>>,
    pub input_hidden: ReadSignal<bool>,
    pub set_input_hidden: WriteSignal<bool>,
    pub dock_collapsed: ReadSignal<bool>,
    set_dock_collapsed: WriteSignal<bool>,
    pub has_prompt_content: RwSignal<bool>,
    pub device_mode: ReadSignal<DeviceMode>,
}

impl MobileState {
    pub fn toggle_sidebar(&self) {
        self.set_sidebar_open.update(|v| *v = !*v);
    }

    pub fn close_sidebar(&self) {
        self.set_sidebar_open.set(false);
    }

    pub fn toggle_panel(&self, panel: MobilePanel) {
        let current = self.active_panel.get_untracked();
        if current == Some(panel) {
            // Closing
            self.set_active_panel.set(None);
            self.set_input_hidden.set(false);
            self.set_dock_collapsed.set(true);
        } else {
            // Opening
            self.set_active_panel.set(Some(panel));
            if panel == MobilePanel::Opencode {
                self.set_input_hidden.set(false);
                self.set_dock_collapsed.set(true);
            } else {
                self.set_dock_collapsed.set(false);
                self.set_input_hidden.set(true);
            }
        }
    }

    pub fn expand_dock(&self) {
        self.set_dock_collapsed.set(false);
        self.set_input_hidden.set(true);
    }

    pub fn collapse_dock(&self) {
        self.set_dock_collapsed.set(true);
    }

    pub fn handle_compose_button_tap(&self) {
        self.set_input_hidden.set(false);
        self.set_dock_collapsed.set(true);
    }

    pub fn handle_scroll_direction(&self, direction: &str) {
        // Only act on phone/tablet
        if self.device_mode.get_untracked().is_desktop() {
            return;
        }

        if direction == "up" {
            // Don't hide if user has typed content in prompt
            if !self.has_prompt_content.get_untracked() {
                self.set_input_hidden.set(true);
                self.set_dock_collapsed.set(true);
            }
        }
        // "down" — do nothing, dock only reappears on FAB tap
    }

    /// Whether compose should be shown in the dock.
    /// On non-chat panels (mobile), compose is hidden from the dock.
    pub fn show_dock_compose(&self) -> bool {
        let hidden = self.input_hidden.get_untracked();
        let collapsed = self.dock_collapsed.get_untracked();
        let panel = self.active_panel.get_untracked();

        if !hidden || collapsed {
            return false;
        }
        // On phone, hide compose in dock when on non-chat panel
        if self.device_mode.get_untracked().is_phone() {
            return panel.is_none() || panel == Some(MobilePanel::Opencode);
        }
        true
    }
}

/// Create mobile/tablet state. Call once at the layout level.
pub fn use_mobile_state() -> MobileState {
    let (sidebar_open, set_sidebar_open) = signal(false);
    let (active_panel, set_active_panel) = signal::<Option<MobilePanel>>(None);
    let (input_hidden, set_input_hidden) = signal(false);
    let (dock_collapsed, set_dock_collapsed) = signal(true);
    let has_prompt_content = RwSignal::new(false);

    // Device mode — reactive, updates on resize
    let (device_mode, set_device_mode) = signal(detect_device_mode());

    // Listen for resize events to update device mode
    Effect::new(move |_| {
        let cb = Closure::<dyn Fn()>::new(move || {
            set_device_mode.set(detect_device_mode());
        });
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref());
        }
        cb.forget();
    });

    MobileState {
        sidebar_open,
        set_sidebar_open,
        active_panel,
        set_active_panel,
        input_hidden,
        set_input_hidden,
        dock_collapsed,
        set_dock_collapsed,
        has_prompt_content,
        device_mode,
    }
}
