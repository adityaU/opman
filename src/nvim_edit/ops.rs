use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use rmpv::Value;

use super::columns::utf16_to_byte;
use super::engine::{EditEngine, MAX_EDIT_ID};
use super::sync::StaleTick;
use crate::nvim_ui::stream::wire::{ControlMsg, ExCommand, TextPosition};

impl EditEngine {
    pub(super) async fn apply_edit(
        self: &Arc<Self>,
        expected: u64,
        start: TextPosition,
        end: TextPosition,
        lines: Vec<String>,
        edit_id: String,
    ) -> Result<()> {
        if edit_id.is_empty() || edit_id.len() > MAX_EDIT_ID {
            bail!("edit_id must be between 1 and {MAX_EDIT_ID} bytes")
        }
        let _rpc = self.rpc_lock.lock().await;
        let (buffer, start_line, start_byte, end_line, end_byte) = {
            let state = self.state.lock().await;
            let document = state.document.as_ref().context("no buffer is attached")?;
            if !document.attached {
                let actual = document.changedtick;
                drop(state);
                self.send(ControlMsg::ResyncRequired {
                    changedtick: actual,
                    reason: "buffer is no longer attached".into(),
                });
                return Ok(());
            }
            if let Err(StaleTick { actual, .. }) = document.require_tick(expected) {
                drop(state);
                self.send(ControlMsg::ResyncRequired {
                    changedtick: actual,
                    reason: "edit was based on a stale changedtick".into(),
                });
                return Ok(());
            }
            let start_line = start.line as usize;
            let end_line = end.line as usize;
            let start_byte = client_column(&document.lines, start_line, start.column)?;
            let end_byte = client_column(&document.lines, end_line, end.column)?;
            if (start_line, start_byte) > (end_line, end_byte) {
                bail!("edit range is backwards")
            }
            (document.buffer, start_line, start_byte, end_line, end_byte)
        };
        self.state
            .lock()
            .await
            .pending_origins
            .push_back(super::engine::PendingOrigin {
                base_tick: expected,
                edit_id,
            });
        if let Err(error) = self
            .call(
                "nvim_buf_set_text",
                vec![
                    Value::from(buffer),
                    Value::from(start_line),
                    Value::from(start_byte),
                    Value::from(end_line),
                    Value::from(end_byte),
                    Value::Array(lines.into_iter().map(Value::from).collect()),
                ],
            )
            .await
        {
            let mut state = self.state.lock().await;
            if let Some(index) = state
                .pending_origins
                .iter()
                .position(|pending| pending.base_tick == expected)
            {
                let _ = state.pending_origins.remove(index);
            }
            return Err(error);
        }
        let changedtick = self.changedtick(buffer).await?;
        if let Some(document) = self.state.lock().await.document.as_mut() {
            document.changedtick = document.changedtick.max(changedtick);
        }
        Ok(())
    }

    /// Follow the caret a pointer put down. Out-of-range positions are the
    /// browser racing a buffer change, not an error worth breaking the session.
    pub(super) async fn move_cursor(self: &Arc<Self>, position: TextPosition) -> Result<()> {
        let byte = {
            let state = self.state.lock().await;
            let document = state.document.as_ref().context("no buffer is attached")?;
            if !document.attached || state.cmdline.is_some() {
                return Ok(());
            }
            match client_column(&document.lines, position.line as usize, position.column) {
                Ok(byte) => byte,
                Err(_) => return Ok(()),
            }
        };
        self.call(
            "nvim_win_set_cursor",
            vec![
                0.into(),
                Value::Array(vec![
                    Value::from(u64::from(position.line) + 1),
                    Value::from(byte),
                ]),
            ],
        )
        .await?;
        self.schedule_state();
        Ok(())
    }

    pub(super) async fn command(self: &Arc<Self>, command: &ExCommand) -> Result<()> {
        let _rpc = self.rpc_lock.lock().await;
        let (buffer, before_tick, line_count) = {
            let state = self.state.lock().await;
            let document = state.document.as_ref().context("no buffer is attached")?;
            if !document.attached {
                bail!("buffer is no longer attached")
            }
            (
                document.buffer,
                document.changedtick,
                Some(document.lines.len()),
            )
        };
        let Value::Array(args) = super::commands::request(command, line_count)? else {
            bail!("command did not produce arguments")
        };
        if matches!(command, ExCommand::BufferDelete) {
            self.state.lock().await.detach_reason = Some("Neovim deleted the buffer".into());
        }
        let output = self.call("nvim_cmd", args).await?;
        let loaded = self
            .call("nvim_buf_is_loaded", vec![Value::from(buffer)])
            .await?
            .as_bool()
            .context("buffer loaded state was not a boolean")?;
        let (changedtick, attached) = if loaded {
            let changedtick = self.changedtick(buffer).await?;
            if command_changes_buffer(command) && changedtick > before_tick {
                self.wait_for_buffer_tick(buffer, changedtick).await;
            }
            (changedtick, !matches!(command, ExCommand::EditReload))
        } else {
            self.wait_for_buffer_detach(buffer).await;
            (before_tick, false)
        };
        self.send(ControlMsg::CommandOutput {
            changedtick,
            output: output.as_str().unwrap_or_default().to_owned(),
        });
        if attached {
            self.schedule_state();
        }
        Ok(())
    }

    async fn wait_for_buffer_tick(&self, buffer: u64, target: u64) {
        for _ in 0..100 {
            let settled = self
                .state
                .lock()
                .await
                .document
                .as_ref()
                .is_some_and(|document| {
                    document.buffer == buffer && document.attached && document.changedtick >= target
                });
            if settled {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        self.resync_with_tick(target).await;
    }

    async fn wait_for_buffer_detach(&self, buffer: u64) {
        for _ in 0..100 {
            let detached = self
                .state
                .lock()
                .await
                .document
                .as_ref()
                .is_none_or(|document| document.buffer != buffer || !document.attached);
            if detached {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let changedtick = {
            let mut state = self.state.lock().await;
            let Some(document) = state.document.as_mut() else {
                return;
            };
            if document.buffer != buffer || !document.attached {
                return;
            }
            document.attached = false;
            document.changedtick
        };
        self.send(ControlMsg::BufferDetached {
            buffer,
            changedtick,
            reason: "Neovim deleted the buffer".into(),
        });
    }
}

fn command_changes_buffer(command: &ExCommand) -> bool {
    matches!(
        command,
        ExCommand::Substitute { .. } | ExCommand::Sort { .. } | ExCommand::Undo | ExCommand::Redo
    )
}

fn client_column(lines: &[String], line: usize, column: u32) -> Result<usize> {
    let value = lines.get(line).context("edit line is outside the buffer")?;
    utf16_to_byte(value, column as usize).map_err(|error| anyhow!(error))
}
