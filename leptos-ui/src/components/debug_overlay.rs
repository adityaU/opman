//! Debug log infrastructure and themed side-panel.
//!
//! `DebugLog` + `dbg_log()` provide a global log sink accessible from both
//! Leptos reactive contexts and plain JS callbacks (thread-local fallback).
//! `DebugPanel` renders the log inside the side-panel system, matching the
//! editor/git panel pattern. Accessible via command palette (Cmd+Shift+/).

use leptos::prelude::*;
use std::cell::RefCell;

/// Max lines kept in the ring buffer.
const MAX_LINES: usize = 200;

thread_local! {
    static GLOBAL_DBG: RefCell<Option<DebugLog>> = const { RefCell::new(None) };
}

/// Global debug log state — provide at app level, push from anywhere.
#[derive(Clone, Copy)]
pub struct DebugLog {
    lines: RwSignal<Vec<String>>,
}

impl DebugLog {
    pub fn new() -> Self {
        let dl = Self {
            lines: RwSignal::new(Vec::with_capacity(MAX_LINES)),
        };
        GLOBAL_DBG.with(|g| *g.borrow_mut() = Some(dl));
        dl
    }

    /// Push a line. Drops oldest if over capacity.
    pub fn push(&self, msg: String) {
        self.lines.update(|v| {
            if v.len() >= MAX_LINES {
                v.remove(0);
            }
            v.push(msg);
        });
        log::info!(
            "{}",
            self.lines
                .with_untracked(|v| v.last().cloned().unwrap_or_default())
        );
    }

    pub fn clear(&self) {
        self.lines.set(Vec::new());
    }
}

/// Push a debug message to the on-screen overlay.
///
/// Fallback tiers:
/// 1. Leptos `use_context` (reactive owner)
/// 2. Thread-local global (JS callbacks without owner)
/// 3. `log::info!` fallback
pub fn dbg_log(msg: &str) {
    if let Some(dl) = use_context::<DebugLog>() {
        dl.push(msg.to_string());
        return;
    }
    let pushed = GLOBAL_DBG.with(|g| {
        if let Some(dl) = *g.borrow() {
            dl.push(msg.to_string());
            true
        } else {
            false
        }
    });
    if pushed {
        return;
    }
    log::info!("[dbg_log no ctx] {}", msg);
}

/// Themed debug panel rendered inside the side-panel system.
#[component]
pub fn DebugPanel() -> impl IntoView {
    let dl = expect_context::<DebugLog>();

    view! {
        <div class="debug-panel">
            <div class="debug-panel-toolbar">
                <button
                    class="debug-panel-clear"
                    on:click=move |_| dl.clear()
                    title="Clear log"
                >
                    "Clear"
                </button>
                <span class="debug-panel-count">
                    {move || format!("{} lines", dl.lines.get().len())}
                </span>
            </div>
            <div class="debug-panel-log">
                {move || dl.lines.get().iter().enumerate().map(|(i, line)| {
                    let line = line.clone();
                    view! {
                        <div class="debug-panel-line">
                            <span class="debug-panel-idx">{format!("{:>3}", i)}</span>
                            <span class="debug-panel-msg">{line}</span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
