//! PTY bridge — spawns PTY processes, wires SSE output to the terminal screen,
//! and sends user input back. Manages per-tab lifecycle (SSE + ResizeObserver).
//!
//! Performance optimisations:
//! - **Output batching**: SSE output events accumulate raw bytes in a buffer;
//!   a single `requestAnimationFrame` processes the entire batch and bumps
//!   the revision signal once per frame.
//! - **Input batching**: rapid keystrokes are coalesced into a single
//!   `pty_write` POST after an 8 ms idle window (flushed immediately on
//!   newline / control chars).

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

// ── Output batching (rAF-based) ────────────────────────────────────

/// Shared state for rAF output batching. Multiple SSE `output` events
/// append bytes here; a single rAF callback drains the buffer once per frame.
struct OutputBatch {
    buf: Vec<u8>,
    raf_pending: bool,
}

/// Schedule a single rAF that will drain `batch`, feed the screen, and
/// bump the revision signal exactly once.
fn schedule_output_flush(
    batch: Rc<RefCell<OutputBatch>>,
    screen: SendWrapper<TermScreen>,
    set_revision: WriteSignal<u64>,
) {
    {
        let mut b = batch.borrow_mut();
        if b.raf_pending {
            return; // Already scheduled.
        }
        b.raf_pending = true;
    }

    let cb = Closure::once(move || {
        let bytes = {
            let mut b = batch.borrow_mut();
            b.raf_pending = false;
            std::mem::take(&mut b.buf)
        };
        if !bytes.is_empty() {
            screen.process(&bytes);
            set_revision.update(|r| *r += 1);
        }
    });
    let _ = web_sys::window()
        .unwrap()
        .request_animation_frame(cb.as_ref().unchecked_ref());
    cb.forget();
}

// ── Input batching ─────────────────────────────────────────────────

/// Flush interval (ms) — rapid keystrokes within this window are coalesced.
const INPUT_FLUSH_MS: i32 = 8;

/// Shared mutable state for input coalescing.
struct InputBatch {
    buf: String,
    timer_id: Option<i32>,
}

/// Append data to the input batch and schedule a flush.
/// If the data ends with a newline or control character, flush immediately.
fn batch_input(batch: &Rc<RefCell<InputBatch>>, tab_id: &str, data: String) {
    let immediate = data.ends_with('\r')
        || data.ends_with('\n')
        || data.starts_with('\x1b')
        || (data.len() == 1 && data.as_bytes()[0] < 0x20);

    {
        let mut b = batch.borrow_mut();
        b.buf.push_str(&data);

        if immediate {
            // Cancel pending timer and flush now.
            if let Some(tid) = b.timer_id.take() {
                web_sys::window().unwrap().clear_timeout_with_handle(tid);
            }
            let payload = std::mem::take(&mut b.buf);
            drop(b);
            flush_input(tab_id, payload);
            return;
        }

        // If a timer is already pending, let it fire.
        if b.timer_id.is_some() {
            return;
        }
    }

    // Schedule a deferred flush.
    let batch_rc = batch.clone();
    let tid_str = tab_id.to_string();
    let cb = Closure::once(move || {
        let payload = {
            let mut b = batch_rc.borrow_mut();
            b.timer_id = None;
            std::mem::take(&mut b.buf)
        };
        if !payload.is_empty() {
            flush_input(&tid_str, payload);
        }
    });
    let window = web_sys::window().unwrap();
    if let Ok(tid) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        INPUT_FLUSH_MS,
    ) {
        batch.borrow_mut().timer_id = Some(tid);
    }
    cb.forget();
}

fn flush_input(tab_id: &str, data: String) {
    let encoded = encode_input_b64(&data);
    let pid = tab_id.to_string();
    leptos::task::spawn_local(async move {
        let _ = crate::api::pty::pty_write(&pid, &encoded).await;
    });
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

                // Wire SSE output → batched vt100 processing
                if let Ok(event_source) = crate::sse::connection::create_pty_sse(&tab_id) {
                    let out_batch = Rc::new(RefCell::new(OutputBatch {
                        buf: Vec::with_capacity(4096),
                        raf_pending: false,
                    }));

                    let scr = screen.clone();
                    let set_rev = set_revision;
                    let ob = out_batch.clone();
                    let output_cb = Closure::<dyn Fn(web_sys::MessageEvent)>::new(
                        move |e: web_sys::MessageEvent| {
                            let data = e.data().as_string().unwrap_or_default();
                            if data.is_empty() {
                                return;
                            }
                            let bytes = decode_output_b64(&data);
                            ob.borrow_mut().buf.extend_from_slice(&bytes);
                            schedule_output_flush(ob.clone(), scr.clone(), set_rev);
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

/// Send user input to the PTY with batching/coalescing.
/// Rapid keystrokes within 8ms are combined into a single POST.
pub fn send_input(tab_id: &str, data: String) {
    thread_local! {
        static BATCHES: RefCell<HashMap<String, Rc<RefCell<InputBatch>>>> =
            RefCell::new(HashMap::new());
    }

    BATCHES.with(|map| {
        let mut map = map.borrow_mut();
        let batch = map
            .entry(tab_id.to_string())
            .or_insert_with(|| {
                Rc::new(RefCell::new(InputBatch {
                    buf: String::new(),
                    timer_id: None,
                }))
            })
            .clone();
        batch_input(&batch, tab_id, data);
    });
}
