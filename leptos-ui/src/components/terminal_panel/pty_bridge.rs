//! PTY bridge — spawns PTY processes, wires SSE output to the terminal screen,
//! and sends user input back. Manages per-tab lifecycle (SSE + ResizeObserver).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use leptos::prelude::*;

use super::native_term::TermScreen;
use super::types::{decode_output_b64, encode_input_b64, TabInfo, TabStatus};

// ── Per-tab runtime ────────────────────────────────────────────────

pub struct TabRuntime {
    pub screen: SendWrapper<TermScreen>,
    pub set_revision: WriteSignal<u64>,
    pub sse: Option<web_sys::EventSource>,
    pub observer: Option<web_sys::ResizeObserver>,
    pub _closures: Vec<Box<dyn std::any::Any>>,
}

impl TabRuntime {
    pub fn cleanup(&mut self) {
        if let Some(obs) = self.observer.take() {
            obs.disconnect();
        }
        if let Some(sse) = self.sse.take() {
            sse.close();
        }
    }
}

pub type Runtimes = Rc<RefCell<HashMap<String, TabRuntime>>>;

/// A cloneable, Send-safe wrapper around Runtimes for Leptos callbacks.
#[derive(Clone)]
pub struct RuntimesHandle(SendWrapper<Runtimes>);

impl RuntimesHandle {
    pub fn new() -> Self {
        Self(SendWrapper::new(Rc::new(RefCell::new(HashMap::new()))))
    }
    pub fn borrow(&self) -> std::cell::Ref<'_, HashMap<String, TabRuntime>> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, HashMap<String, TabRuntime>> {
        self.0.borrow_mut()
    }
    pub fn inner(&self) -> &Runtimes {
        &self.0
    }
}

// ── Tab initialization ─────────────────────────────────────────────

/// Initialize a terminal tab: create screen, spawn PTY, wire SSE output.
pub fn init_tab(
    tab_id: String,
    kind: String,
    session_id_val: Option<String>,
    set_tabs: WriteSignal<Vec<TabInfo>>,
    runtimes: Runtimes,
    screen: SendWrapper<TermScreen>,
    set_revision: WriteSignal<u64>,
) {
    leptos::task::spawn_local(async move {
        // Wait two frames for DOM
        for _ in 0..2 {
            let p = js_sys::Promise::new(&mut |resolve, _| {
                let _ = web_sys::window().unwrap().request_animation_frame(&resolve);
            });
            let _ = wasm_bindgen_futures::JsFuture::from(p).await;
        }

        let rows = screen.rows();
        let cols = screen.cols();
        let mut closures: Vec<Box<dyn std::any::Any>> = Vec::new();

        // Spawn PTY
        let sid_opt = if kind == "opencode" {
            session_id_val.as_deref()
        } else {
            None
        };

        let spawn_result =
            crate::api::pty::pty_spawn(&kind, &tab_id, rows, cols, sid_opt).await;

        let mut sse_handle: Option<web_sys::EventSource> = None;

        match spawn_result {
            Ok(_) => {
                set_tabs.update(|ts| {
                    if let Some(t) = ts.iter_mut().find(|t| t.id == tab_id) {
                        t.status = TabStatus::Ready;
                    }
                });

                // Wire SSE output → vt100 parser
                if let Ok(event_source) = crate::sse::connection::create_pty_sse(&tab_id) {
                    let scr = screen.clone();
                    let set_rev = set_revision;
                    let output_cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
                        move |e: web_sys::MessageEvent| {
                            let data = e.data().as_string().unwrap_or_default();
                            if data.is_empty() {
                                return;
                            }
                            let bytes = decode_output_b64(&data);
                            scr.process(&bytes);
                            set_rev.update(|r| *r += 1);
                        },
                    );
                    let _ = event_source.add_event_listener_with_callback(
                        "output",
                        output_cb.as_ref().unchecked_ref(),
                    );
                    closures.push(Box::new(output_cb));

                    let err_cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
                        move |e: web_sys::MessageEvent| {
                            log::error!(
                                "PTY SSE error: {}",
                                e.data().as_string().unwrap_or_default()
                            );
                        },
                    );
                    let _ = event_source.add_event_listener_with_callback(
                        "error",
                        err_cb.as_ref().unchecked_ref(),
                    );
                    closures.push(Box::new(err_cb));
                    sse_handle = Some(event_source);
                }
            }
            Err(e) => {
                log::error!("Failed to spawn PTY for tab {}: {}", tab_id, e);
                set_tabs.update(|ts| {
                    if let Some(t) = ts.iter_mut().find(|t| t.id == tab_id) {
                        t.status = TabStatus::Error;
                    }
                });
                // Write error message to the screen
                screen.process(
                    b"\r\n\x1b[31mFailed to spawn terminal process.\x1b[0m\r\n",
                );
                set_revision.update(|r| *r += 1);
            }
        }

        runtimes.borrow_mut().insert(
            tab_id,
            TabRuntime {
                screen,
                set_revision,
                sse: sse_handle,
                observer: None,
                _closures: closures,
            },
        );
    });
}

/// Send user input to the PTY (base64-encoded, fire-and-forget).
pub fn send_input(tab_id: &str, data: String) {
    let encoded = encode_input_b64(&data);
    let pid = tab_id.to_string();
    leptos::task::spawn_local(async move {
        let _ = crate::api::pty::pty_write(&pid, &encoded).await;
    });
}
