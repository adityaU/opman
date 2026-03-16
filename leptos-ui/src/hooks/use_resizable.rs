//! Resizable panel hook.
//! Matches React `useResizable.ts` behavior — mouse + touch support.
//! Uses a clean closure-based approach for Leptos CSR.

use leptos::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ResizeDirection {
    Horizontal,
    Vertical,
}

/// Configuration for a resizable panel.
pub struct ResizableOptions {
    pub initial_size: f64,
    pub min_size: f64,
    pub max_size: f64,
    pub direction: ResizeDirection,
    pub reverse: bool,
}

impl Default for ResizableOptions {
    fn default() -> Self {
        Self {
            initial_size: 280.0,
            min_size: 150.0,
            max_size: 800.0,
            direction: ResizeDirection::Horizontal,
            reverse: false,
        }
    }
}

/// State returned by use_resizable.
#[derive(Clone, Copy)]
pub struct ResizableState {
    /// Current size in pixels.
    pub size: ReadSignal<f64>,
    pub set_size: WriteSignal<f64>,
    /// Whether currently dragging.
    pub is_dragging: ReadSignal<bool>,
    set_is_dragging: WriteSignal<bool>,
    /// Direction of resize.
    pub direction: ResizeDirection,
    /// Shared drag start position.
    start_pos: StoredValue<f64>,
    /// Shared drag start size.
    start_size: StoredValue<f64>,
    /// Min size.
    min_size: f64,
    /// Max size.
    max_size: f64,
    /// Reverse direction.
    reverse: bool,
}

impl ResizableState {
    /// CSS pixel string for the current size.
    pub fn size_px(&self) -> String {
        format!("{}px", self.size.get_untracked())
    }

    /// Call this from the mousedown handler on the drag handle.
    pub fn start_drag(&self, e: web_sys::MouseEvent) {
        e.prevent_default();
        e.stop_propagation();

        let pos = match self.direction {
            ResizeDirection::Horizontal => e.client_x() as f64,
            ResizeDirection::Vertical => e.client_y() as f64,
        };

        self.start_pos.set_value(pos);
        self.start_size.set_value(self.size.get_untracked());
        self.set_is_dragging.set(true);

        // Install temporary global listeners for this drag operation
        let set_size = self.set_size;
        let set_is_dragging = self.set_is_dragging;
        let direction = self.direction;
        let reverse = self.reverse;
        let min_size = self.min_size;
        let max_size = self.max_size;
        let sp = self.start_pos;
        let ss = self.start_size;

        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("no document");

        // Prevent text selection and set cursor during drag
        if let Some(body) = document.body() {
            let _ = body.style().set_property("user-select", "none");
            let cursor = match direction {
                ResizeDirection::Horizontal => "col-resize",
                ResizeDirection::Vertical => "row-resize",
            };
            let _ = body.style().set_property("cursor", cursor);
        }

        // We need Rc<Cell<>> to share closures between move and up handlers
        let move_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::MouseEvent)>>>> =
            Rc::new(Cell::new(None));
        let up_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::MouseEvent)>>>> =
            Rc::new(Cell::new(None));
        let touch_move_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
            Rc::new(Cell::new(None));
        let touch_end_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
            Rc::new(Cell::new(None));
        let touch_cancel_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
            Rc::new(Cell::new(None));

        let doc_clone = document.clone();
        let move_cb_clone = move_cb.clone();
        let up_cb_clone = up_cb.clone();
        let touch_move_clone = touch_move_cb.clone();
        let touch_end_clone = touch_end_cb.clone();
        let touch_cancel_clone = touch_cancel_cb.clone();

        // Cleanup function
        let cleanup = Rc::new(move || {
            set_is_dragging.set(false);

            // Restore body styles
            if let Some(body) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.body())
            {
                let _ = body.style().remove_property("user-select");
                let _ = body.style().remove_property("cursor");
            }

            // Remove event listeners
            if let Some(cb) = move_cb_clone.take() {
                let _ = doc_clone
                    .remove_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref());
            }
            if let Some(cb) = up_cb_clone.take() {
                let _ = doc_clone
                    .remove_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref());
            }
            if let Some(cb) = touch_move_clone.take() {
                let _ = doc_clone
                    .remove_event_listener_with_callback("touchmove", cb.as_ref().unchecked_ref());
            }
            if let Some(cb) = touch_end_clone.take() {
                let _ = doc_clone
                    .remove_event_listener_with_callback("touchend", cb.as_ref().unchecked_ref());
            }
            if let Some(cb) = touch_cancel_clone.take() {
                let _ = doc_clone.remove_event_listener_with_callback(
                    "touchcancel",
                    cb.as_ref().unchecked_ref(),
                );
            }
        });

        // Mouse move handler
        let on_move = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            let current_pos = match direction {
                ResizeDirection::Horizontal => e.client_x() as f64,
                ResizeDirection::Vertical => e.client_y() as f64,
            };
            let delta = current_pos - sp.get_value();
            let new_size = if reverse {
                ss.get_value() - delta
            } else {
                ss.get_value() + delta
            };
            set_size.set(new_size.max(min_size).min(max_size));
        });

        // Mouse up handler
        let cleanup_for_up = cleanup.clone();
        let on_up = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            cleanup_for_up();
        });

        // Touch move handler
        let on_touch_move =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |e: web_sys::TouchEvent| {
                if let Some(touch) = e.touches().item(0) {
                    let current_pos = match direction {
                        ResizeDirection::Horizontal => touch.client_x() as f64,
                        ResizeDirection::Vertical => touch.client_y() as f64,
                    };
                    let delta = current_pos - sp.get_value();
                    let new_size = if reverse {
                        ss.get_value() - delta
                    } else {
                        ss.get_value() + delta
                    };
                    set_size.set(new_size.max(min_size).min(max_size));
                }
            });

        // Touch end handler
        let cleanup_for_up = cleanup.clone();
        let on_up = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            cleanup_for_up();
        });

        // Touch move handler
        let on_touch_move =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |e: web_sys::TouchEvent| {
                if let Some(touch) = e.touches().item(0) {
                    let current_pos = match direction {
                        ResizeDirection::Horizontal => touch.client_x() as f64,
                        ResizeDirection::Vertical => touch.client_y() as f64,
                    };
                    let delta = current_pos - sp.get_value();
                    let new_size = if reverse {
                        ss.get_value() - delta
                    } else {
                        ss.get_value() + delta
                    };
                    set_size.set(new_size.max(min_size).min(max_size));
                }
            });

        // Touch end handler
        let cleanup_for_touch = cleanup.clone();
        let on_touch_end =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_e: web_sys::TouchEvent| {
                cleanup_for_touch();
            });

        // Touch cancel handler
        let cleanup_for_cancel = cleanup;
        let on_touch_cancel =
            Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_e: web_sys::TouchEvent| {
                cleanup_for_cancel();
            });

        // Register listeners
        let _ = document
            .add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref());
        let _ =
            document.add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref());
        let _ = document
            .add_event_listener_with_callback("touchmove", on_touch_move.as_ref().unchecked_ref());
        let _ = document
            .add_event_listener_with_callback("touchend", on_touch_end.as_ref().unchecked_ref());
        let _ = document.add_event_listener_with_callback(
            "touchcancel",
            on_touch_cancel.as_ref().unchecked_ref(),
        );

        // Store closures so cleanup can remove them
        move_cb.set(Some(on_move));
        up_cb.set(Some(on_up));
        touch_move_cb.set(Some(on_touch_move));
        touch_end_cb.set(Some(on_touch_end));
        touch_cancel_cb.set(Some(on_touch_cancel));
    }

    /// Call this from the touchstart handler on the drag handle.
    pub fn start_drag_touch(&self, e: web_sys::TouchEvent) {
        e.prevent_default();
        e.stop_propagation();

        if let Some(touch) = e.touches().item(0) {
            let pos = match self.direction {
                ResizeDirection::Horizontal => touch.client_x() as f64,
                ResizeDirection::Vertical => touch.client_y() as f64,
            };

            self.start_pos.set_value(pos);
            self.start_size.set_value(self.size.get_untracked());
            self.set_is_dragging.set(true);

            // Reuse the same mechanism — install global listeners
            let set_size = self.set_size;
            let set_is_dragging = self.set_is_dragging;
            let direction = self.direction;
            let reverse = self.reverse;
            let min_size = self.min_size;
            let max_size = self.max_size;
            let sp = self.start_pos;
            let ss = self.start_size;

            let document = web_sys::window()
                .and_then(|w| w.document())
                .expect("no document");

            let doc_clone = document.clone();
            let touch_move_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
                Rc::new(Cell::new(None));
            let touch_end_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
                Rc::new(Cell::new(None));
            let touch_cancel_cb: Rc<Cell<Option<Closure<dyn Fn(web_sys::TouchEvent)>>>> =
                Rc::new(Cell::new(None));
            let move_clone = touch_move_cb.clone();
            let end_clone = touch_end_cb.clone();
            let cancel_clone = touch_cancel_cb.clone();

            let cleanup = Rc::new(move || {
                set_is_dragging.set(false);
                if let Some(cb) = move_clone.take() {
                    let _ = doc_clone.remove_event_listener_with_callback(
                        "touchmove",
                        cb.as_ref().unchecked_ref(),
                    );
                }
                if let Some(cb) = end_clone.take() {
                    let _ = doc_clone.remove_event_listener_with_callback(
                        "touchend",
                        cb.as_ref().unchecked_ref(),
                    );
                }
                if let Some(cb) = cancel_clone.take() {
                    let _ = doc_clone.remove_event_listener_with_callback(
                        "touchcancel",
                        cb.as_ref().unchecked_ref(),
                    );
                }
            });

            let on_move =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |e: web_sys::TouchEvent| {
                    if let Some(touch) = e.touches().item(0) {
                        let current_pos = match direction {
                            ResizeDirection::Horizontal => touch.client_x() as f64,
                            ResizeDirection::Vertical => touch.client_y() as f64,
                        };
                        let delta = current_pos - sp.get_value();
                        let new_size = if reverse {
                            ss.get_value() - delta
                        } else {
                            ss.get_value() + delta
                        };
                        set_size.set(new_size.max(min_size).min(max_size));
                    }
                });

            let cleanup_for_end = cleanup.clone();
            let on_end =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_e: web_sys::TouchEvent| {
                    cleanup_for_end();
                });

            let cleanup_for_cancel = cleanup;
            let on_cancel =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_e: web_sys::TouchEvent| {
                    cleanup_for_cancel();
                });

            let _ = document
                .add_event_listener_with_callback("touchmove", on_move.as_ref().unchecked_ref());
            let _ = document
                .add_event_listener_with_callback("touchend", on_end.as_ref().unchecked_ref());
            let _ = document.add_event_listener_with_callback(
                "touchcancel",
                on_cancel.as_ref().unchecked_ref(),
            );

            touch_move_cb.set(Some(on_move));
            touch_end_cb.set(Some(on_end));
            touch_cancel_cb.set(Some(on_cancel));
        }
    }
}

/// Hook for making a panel resizable via a drag handle.
pub fn use_resizable(opts: ResizableOptions) -> ResizableState {
    let (size, set_size) = signal(opts.initial_size);
    let (is_dragging, set_is_dragging) = signal(false);

    let start_pos = StoredValue::new(0.0_f64);
    let start_size = StoredValue::new(0.0_f64);

    ResizableState {
        size,
        set_size,
        is_dragging,
        set_is_dragging,
        direction: opts.direction,
        start_pos,
        start_size,
        min_size: opts.min_size,
        max_size: opts.max_size,
        reverse: opts.reverse,
    }
}
