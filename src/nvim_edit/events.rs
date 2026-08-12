use std::sync::Arc;

use rmpv::Value;
use tokio::sync::broadcast;

use super::engine::EditEngine;
use super::notifications::{self, Notification};
use crate::nvim_ui::stream::wire::ControlMsg;
use crate::nvim_ui::NvimNotification;

impl EditEngine {
    pub(crate) async fn notifications(
        self: Arc<Self>,
        mut receiver: broadcast::Receiver<NvimNotification>,
    ) {
        loop {
            let notification = match receiver.recv().await {
                Ok(value) => value,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    self.resync("notification stream lagged").await;
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return,
            };
            for event in notifications::decode(&notification) {
                self.event(event).await;
            }
        }
    }

    async fn event(self: &Arc<Self>, event: Notification) {
        match event {
            Notification::Lines {
                buffer,
                changedtick,
                first_line,
                last_line,
                new_last_line,
                lines,
            } => {
                self.lines_event(
                    buffer,
                    changedtick,
                    first_line,
                    last_line,
                    new_last_line,
                    lines,
                )
                .await
            }
            Notification::Detached { buffer } => self.detach_event(buffer).await,
            Notification::Reloaded {
                buffer,
                changedtick,
            } if self.is_current(buffer).await => {
                let tick = match changedtick {
                    Some(tick) => tick,
                    None => self.current_tick().await,
                };
                self.resync_with_tick(tick).await;
            }
            Notification::Reloaded { .. } => {}
            Notification::Message { kind, text } => self.send(ControlMsg::Message {
                changedtick: self.current_tick().await,
                kind,
                text,
            }),
            Notification::CmdlineShow {
                content,
                position,
                first_char,
            } => {
                self.state.lock().await.cmdline = Some((first_char.clone(), content.clone()));
                self.send(ControlMsg::Cmdline {
                    visible: true,
                    first_char,
                    content,
                    position,
                });
            }
            Notification::CmdlinePos { position } => {
                let Some((first_char, content)) = self.state.lock().await.cmdline.clone() else {
                    return;
                };
                self.send(ControlMsg::Cmdline {
                    visible: true,
                    first_char,
                    content,
                    position,
                });
            }
            Notification::CmdlineHide => {
                self.state.lock().await.cmdline = None;
                self.send(ControlMsg::Cmdline {
                    visible: false,
                    first_char: String::new(),
                    content: String::new(),
                    position: 0,
                });
            }
            // Neovim has caught up. Release exactly one browser input now,
            // after the key has been processed, then ask for the resulting
            // state. A response to nvim_input only says that Neovim accepted
            // the bytes into its input queue; using it as an ACK can reorder
            // adjacent keys and lose characters in command-line input.
            Notification::Flush => {
                self.send(ControlMsg::InputAck {});
                self.schedule_state();
            }
            Notification::Action { name } => self.send(ControlMsg::Action { name }),
        }
    }

    async fn lines_event(
        self: &Arc<Self>,
        buffer: u64,
        tick: u64,
        first: usize,
        last: usize,
        new_last: usize,
        lines: Vec<String>,
    ) {
        let mut state = self.state.lock().await;
        if state.initializing {
            state.ignore_initial_lines = false;
            return;
        }
        if state
            .document
            .as_ref()
            .is_none_or(|document| document.buffer != buffer)
        {
            return;
        }
        if state.ignore_initial_lines {
            state.ignore_initial_lines = false;
            return;
        }
        let origin = state
            .pending_origins
            .front()
            .filter(|pending| pending.base_tick.saturating_add(1) == tick)
            .map(|pending| pending.edit_id.clone());
        if origin.is_some() {
            let _ = state.pending_origins.pop_front();
        }
        let applied = state.document.as_mut().is_some_and(|document| {
            document
                .apply_lines(tick, first, last, lines.clone())
                .is_ok()
        });
        if !applied {
            drop(state);
            self.resync_with_tick(tick).await;
            return;
        }
        self.send(ControlMsg::BufferChanged {
            buffer,
            changedtick: tick,
            first_line: first as u32,
            last_line: last as u32,
            new_last_line: new_last as u32,
            lines,
            origin,
        });
        self.schedule_state();
    }

    async fn detach_event(&self, buffer: u64) {
        {
            let state = self.state.lock().await;
            let Some(document) = state.document.as_ref() else {
                return;
            };
            if state.initializing || document.buffer != buffer || !document.attached {
                return;
            }
        }
        let loaded = self
            .call("nvim_buf_is_loaded", vec![Value::from(buffer)])
            .await
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let mut state = self.state.lock().await;
        let Some(document) = state.document.as_ref() else {
            return;
        };
        if document.buffer != buffer || !document.attached {
            return;
        }
        let changedtick = document.changedtick;
        let reason = state
            .detach_reason
            .take()
            .or_else(|| (!loaded).then_some("Neovim deleted the buffer".into()))
            .unwrap_or_else(|| "Neovim detached the buffer".into());
        if let Some(document) = state.document.as_mut() {
            document.attached = false;
        }
        self.send(ControlMsg::BufferDetached {
            buffer,
            changedtick,
            reason,
        });
    }
}
