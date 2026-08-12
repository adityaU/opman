use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use rmpv::Value;
use tokio::time::Duration;

use super::columns::byte_to_utf16;
use super::engine::EditEngine;
use super::snapshot::{Snapshot, SNAPSHOT_LUA};
use crate::nvim_ui::stream::wire::{ControlMsg, TextPosition, VisualSelection};

impl EditEngine {
    pub(super) fn safe_path(&self, path: &str) -> Result<PathBuf> {
        if path.is_empty() {
            bail!("file path is empty")
        }
        let candidate = Path::new(path);
        let target = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.project_dir.join(candidate)
        };
        let root = self
            .project_dir
            .canonicalize()
            .context("project directory is unavailable")?;
        let resolved = if target.exists() {
            target.canonicalize()?
        } else {
            target
                .parent()
                .context("file has no parent")?
                .canonicalize()?
                .join(target.file_name().context("file has no name")?)
        };
        if !resolved.starts_with(&root) {
            bail!("file path is outside the project")
        }
        Ok(resolved)
    }

    /// Ask Neovim for its state. Called on `flush`, which is Neovim's own
    /// statement that it has finished processing whatever it was sent.
    pub(super) fn schedule_state(self: &Arc<Self>) {
        let generation = self.input_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2)).await;
            if let Err(error) = engine.emit_state(generation).await {
                engine.send(ControlMsg::Error {
                    message: error.to_string(),
                });
            }
        });
    }

    async fn emit_state(&self, generation: u64) -> Result<()> {
        let _rpc = self.rpc_lock.lock().await;
        if self.input_generation.load(Ordering::Acquire) != generation {
            return Ok(());
        }
        let (lines, changedtick) = {
            let state = self.state.lock().await;
            let document = state.document.as_ref().context("no buffer is attached")?;
            (document.lines.clone(), document.changedtick)
        };
        let value = self
            .call(
                "nvim_exec_lua",
                vec![Value::from(SNAPSHOT_LUA), Value::Array(Vec::new())],
            )
            .await?;
        let snapshot = Snapshot::parse(&value, self.project_prefix().as_deref())
            .context("Neovim state snapshot was malformed")?;
        if self.input_generation.load(Ordering::Acquire) != generation {
            return Ok(());
        }
        let line = lines
            .get(snapshot.row)
            .map(String::as_str)
            .context("cursor row was outside the attached buffer")?;
        let visual = match snapshot.anchor.filter(|_| snapshot.mode.is_visual()) {
            Some(anchor) => Some(selection(
                &lines,
                anchor,
                (snapshot.row, snapshot.byte),
                snapshot.mode == crate::nvim_ui::stream::wire::NvimMode::VisualLine,
            )?),
            None => None,
        };
        self.send(ControlMsg::State {
            changedtick,
            cursor: TextPosition {
                line: snapshot.row as u32,
                column: byte_to_utf16(line, snapshot.byte)
                    .context("cursor byte column was invalid")? as u32,
            },
            mode: snapshot.mode,
            mode_short: snapshot.mode.short(),
            visual,
        });
        self.publish_search(snapshot.search).await;
        self.publish_layout(snapshot.layout).await;
        Ok(())
    }

    /// Republish only on a change: both surfaces redraw on every flush
    /// otherwise, and a flush happens on every keystroke.
    async fn publish_search(&self, pattern: Option<String>) {
        let mut state = self.state.lock().await;
        if state.search_pattern == pattern {
            return;
        }
        state.search_pattern = pattern.clone();
        drop(state);
        self.send(ControlMsg::Search { pattern });
    }

    async fn publish_layout(&self, layout: crate::nvim_ui::stream::wire::Layout) {
        let mut state = self.state.lock().await;
        if state.layout.as_ref() == Some(&layout) {
            return;
        }
        state.layout = Some(layout.clone());
        drop(state);
        self.send(ControlMsg::Layout { layout });
    }

    pub(super) fn project_prefix(&self) -> Option<String> {
        let mut prefix = self.project_dir.to_string_lossy().into_owned();
        if prefix.is_empty() {
            return None;
        }
        if !prefix.ends_with('/') {
            prefix.push('/');
        }
        Some(prefix)
    }
}

fn selection(
    lines: &[String],
    anchor: (usize, usize),
    cursor: (usize, usize),
    linewise: bool,
) -> Result<VisualSelection> {
    let (start, end) = if anchor <= cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };
    let (start, end) = if linewise {
        ((start.0, 0), (end.0, 0))
    } else {
        (start, end)
    };
    Ok(VisualSelection {
        start: position(lines, start.0, start.1)?,
        end: position(lines, end.0, end.1)?,
    })
}

fn position(lines: &[String], line: usize, byte: usize) -> Result<TextPosition> {
    Ok(TextPosition {
        line: line as u32,
        column: byte_to_utf16(
            lines
                .get(line)
                .map(String::as_str)
                .context("visual mark row was outside the buffer")?,
            byte,
        )
        .context("visual mark byte column was invalid")? as u32,
    })
}
