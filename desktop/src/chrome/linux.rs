//! A native GTK header bar floating over the webview.
//!
//! The window's content box is moved into a `gtk::Overlay`. A 10px input-only
//! strip at the top reveals the header bar; leaving the bar hides it again.
//! Everything goes through GTK, so this works on X11 and Wayland alike.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::glib::{self, ControlFlow};
use gtk::prelude::*;
use tauri::{AppHandle, WebviewWindow, WebviewWindowBuilder, Wry};

use super::bar::{step, Bar, Step};

const REVEAL_STRIP: i32 = 10;
const POLL: Duration = Duration::from_millis(50);
const CSS: &str = "
.opman-titlebar, .opman-titlebar:backdrop {
    background: transparent;
    box-shadow: none;
    border: none;
}";

pub fn prepare(
    builder: WebviewWindowBuilder<'_, Wry, AppHandle>,
) -> WebviewWindowBuilder<'_, Wry, AppHandle> {
    builder.decorations(false)
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    let gtk_window = window.gtk_window()?;
    let content = window.default_vbox()?;
    gtk_window.remove(&content);

    let overlay = gtk::Overlay::new();
    overlay.add(&content);

    let strip = gtk::EventBox::new();
    strip.set_visible_window(false);
    strip.set_size_request(-1, REVEAL_STRIP);
    strip.set_valign(gtk::Align::Start);

    let header = header_bar();
    // HeaderBar has no window of its own; the event box lets presses on its
    // empty space move the window.
    let bar = gtk::EventBox::new();
    bar.set_visible_window(false);
    bar.set_valign(gtk::Align::Start);
    bar.add(&header);

    overlay.add_overlay(&strip);
    overlay.add_overlay(&bar);
    gtk_window.add(&overlay);
    overlay.show_all();
    bar.hide();

    wire(&gtk_window, &strip, &bar);
    Ok(())
}

fn header_bar() -> gtk::HeaderBar {
    let header = gtk::HeaderBar::new();
    header.set_show_close_button(true);
    // GNOME's default layout shows only close; the bar must offer all three.
    header.set_decoration_layout(Some(":minimize,maximize,close"));
    header.style_context().add_class("opman-titlebar");
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(CSS.as_bytes()).is_ok() {
        header
            .style_context()
            .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    header
}

/// Hover state shared by the GTK signal handlers (all on the main thread).
struct Hover {
    bar: Cell<Bar>,
    in_window: Cell<bool>,
}

fn wire(window: &gtk::ApplicationWindow, strip: &gtk::EventBox, bar: &gtk::EventBox) {
    let hover = Rc::new(Hover {
        bar: Cell::new(Bar::Hidden),
        in_window: Cell::new(false),
    });
    window.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);

    window.connect_enter_notify_event({
        let hover = Rc::clone(&hover);
        move |_, _| {
            hover.in_window.set(true);
            glib::Propagation::Proceed
        }
    });
    window.connect_leave_notify_event({
        let hover = Rc::clone(&hover);
        move |_, event| {
            // Moving onto a child widget is not leaving the window.
            if event.detail() != gdk::NotifyType::Inferior {
                hover.in_window.set(false);
            }
            glib::Propagation::Proceed
        }
    });

    strip.connect_enter_notify_event({
        let (window, bar) = (window.clone(), bar.clone());
        move |_, _| {
            hover.in_window.set(true);
            if matches!(hover.bar.get(), Bar::Hidden) {
                bar.show();
                hover.bar.set(Bar::Shown);
                track(&window, &bar, Rc::clone(&hover));
            }
            glib::Propagation::Proceed
        }
    });

    bar.connect_button_press_event({
        let window = window.clone();
        move |_, event| {
            if event.button() != 1 {
                return glib::Propagation::Proceed;
            }
            if event.event_type() == gdk::EventType::DoubleButtonPress {
                toggle_maximize(&window);
                return glib::Propagation::Stop;
            }
            let (x, y) = event.root();
            window.begin_move_drag(1, x as i32, y as i32, event.time());
            glib::Propagation::Stop
        }
    });
}

/// Polls the pointer while the bar is shown. Its buttons have input windows of
/// their own, so enter/leave on the bar cannot tell "on a button" from "gone".
fn track(window: &gtk::ApplicationWindow, bar: &gtk::EventBox, hover: Rc<Hover>) {
    let (window, bar) = (window.clone(), bar.clone());
    glib::timeout_add_local(POLL, move || {
        let state = hover.bar.get();
        if matches!(state, Bar::Hidden) {
            return ControlFlow::Break;
        }
        let inside = hover.in_window.get() && pointer_over(&window, &bar);
        match step(state, inside, Instant::now()) {
            Step::Stay(next) => {
                hover.bar.set(next);
                ControlFlow::Continue
            }
            Step::Show => ControlFlow::Continue,
            Step::Hide => {
                bar.hide();
                hover.bar.set(Bar::Hidden);
                ControlFlow::Break
            }
        }
    });
}

fn pointer_over(window: &gtk::ApplicationWindow, bar: &gtk::EventBox) -> bool {
    let Some(pointer) = gdk::Display::default()
        .and_then(|display| display.default_seat())
        .and_then(|seat| seat.pointer())
    else {
        return false;
    };
    let Some(gdk_window) = window.window() else {
        return false;
    };
    let Some((left, top)) = bar.translate_coordinates(window, 0, 0) else {
        return false;
    };
    let (_, x, y, _) = gdk_window.device_position(&pointer);
    let size = bar.allocation();
    x >= left && x < left + size.width() && y >= top && y < top + size.height()
}

fn toggle_maximize(window: &gtk::ApplicationWindow) {
    if window.is_maximized() {
        window.unmaximize();
    } else {
        window.maximize();
    }
}
