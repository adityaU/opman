//! Native traffic lights, hidden and shown in place. The title bar is a
//! transparent overlay, so hiding them leaves nothing above the page.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSWindowButton};
use tauri::WebviewWindow;

const BUTTONS: [NSWindowButton; 3] = [
    NSWindowButton::CloseButton,
    NSWindowButton::MiniaturizeButton,
    NSWindowButton::ZoomButton,
];

pub fn traffic_lights(window: &WebviewWindow, visible: bool) -> tauri::Result<()> {
    window.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // The app has a single window; panels and menus have no standard buttons.
        for ns_window in NSApplication::sharedApplication(mtm).windows().iter() {
            for kind in BUTTONS {
                if let Some(button) = ns_window.standardWindowButton(kind) {
                    button.setHidden(!visible);
                }
            }
        }
    })
}
