//! CodeMirror-facing Neovim edit engine.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use rmpv::Value;
use tokio::sync::{mpsc, Mutex};

use super::commands;
use super::sync::Document;
use crate::nvim_ui::stream::wire::{ClientMsg, ControlMsg};
use crate::nvim_ui::{NvimClient, NvimSession};

pub(super) const MAX_EDIT_ID: usize = 128;

pub(super) struct EngineState {
    pub(super) document: Option<Document>,
    pub(super) pending_origins: VecDeque<PendingOrigin>,
    pub(super) initializing: bool,
    pub(super) ignore_initial_lines: bool,
    /// The command line Neovim last drew, so a `cmdline_pos` that arrives on
    /// its own can be republished without losing the text.
    pub(super) cmdline: Option<(String, String)>,
    pub(super) search_pattern: Option<String>,
    /// Last layout published, so a flush that changed nothing sends nothing.
    pub(super) layout: Option<crate::nvim_ui::stream::wire::Layout>,
    pub(super) detach_reason: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PendingOrigin {
    pub(super) base_tick: u64,
    pub(super) edit_id: String,
}

pub(crate) struct EditEngine {
    pub(super) client: NvimClient,
    pub(super) session: Arc<NvimSession>,
    pub(super) project_dir: PathBuf,
    pub(super) controls: mpsc::UnboundedSender<ControlMsg>,
    pub(super) state: Mutex<EngineState>,
    pub(super) rpc_lock: Mutex<()>,
    pub(super) input_generation: AtomicU64,
}

impl EditEngine {
    pub(crate) fn new(
        session: Arc<NvimSession>,
        controls: mpsc::UnboundedSender<ControlMsg>,
    ) -> Arc<Self> {
        Arc::new(Self {
            client: session.client(),
            project_dir: session.project_dir().to_path_buf(),
            session,
            controls,
            state: Mutex::new(EngineState {
                document: None,
                pending_origins: VecDeque::new(),
                initializing: false,
                ignore_initial_lines: false,
                cmdline: None,
                search_pattern: None,
                layout: None,
                detach_reason: None,
            }),
            rpc_lock: Mutex::new(()),
            input_generation: AtomicU64::new(0),
        })
    }

    pub(crate) async fn attach(self: &Arc<Self>, path: String) -> Result<()> {
        let path = self.safe_path(&path)?;
        let _rpc = self.rpc_lock.lock().await;
        let old_buffer = self.state.lock().await.document.as_ref().map(|d| d.buffer);
        {
            let mut state = self.state.lock().await;
            state.initializing = true;
            state.ignore_initial_lines = true;
            state.pending_origins.clear();
            state.cmdline = None;
            state.search_pattern = None;
            state.layout = None;
            state.detach_reason = None;
        }
        if let Some(buffer) = old_buffer {
            let _ = self
                .call("nvim_buf_detach", vec![Value::from(buffer)])
                .await;
        }
        let Value::Array(args) = commands::open_request(&path.to_string_lossy())? else {
            bail!("open command did not produce arguments")
        };
        self.call("nvim_cmd", args).await?;
        let buffer = value_u64(&self.call("nvim_get_current_buf", Vec::new()).await?)
            .context("current buffer was not an integer")?;
        self.call(
            "nvim_buf_attach",
            vec![Value::from(buffer), true.into(), Value::Map(Vec::new())],
        )
        .await?;
        let lines = self.lines(buffer).await?;
        let changedtick = self.changedtick(buffer).await?;
        {
            let mut state = self.state.lock().await;
            state.document = Some(Document::new(
                buffer,
                path.to_string_lossy().into_owned(),
                changedtick,
                lines.clone(),
            ));
            state.initializing = false;
        }
        self.send(ControlMsg::Attached {
            buffer,
            path: path.to_string_lossy().into_owned(),
            changedtick,
            lines,
        });
        self.install_actions().await;
        self.schedule_state();
        Ok(())
    }

    pub(crate) async fn handle(self: &Arc<Self>, message: ClientMsg) -> Result<()> {
        match message {
            ClientMsg::Attach { path } => self.attach(path).await,
            ClientMsg::Edit {
                changedtick,
                start,
                end,
                lines,
                edit_id,
            } => {
                self.apply_edit(changedtick, start, end, lines, edit_id)
                    .await
            }
            ClientMsg::Input { keys } => self.input("nvim_input", vec![Value::from(keys)]).await,
            ClientMsg::Cursor { position } => self.move_cursor(position).await,
            ClientMsg::Paste { data } => {
                self.input(
                    "nvim_paste",
                    vec![Value::from(data), false.into(), (-1).into()],
                )
                .await
            }
            ClientMsg::InputMouse {
                button,
                action,
                modifier,
                grid,
                row,
                col,
            } => {
                self.input(
                    "nvim_input_mouse",
                    vec![
                        Value::from(button),
                        Value::from(action),
                        Value::from(modifier),
                        Value::from(grid),
                        Value::from(row),
                        Value::from(col),
                    ],
                )
                .await
            }
            ClientMsg::Resize { .. } => bail!("resize is not part of the edit-engine protocol"),
            ClientMsg::Command { command } => self.command(&command).await,
        }
    }

    pub(super) async fn call(&self, method: &str, args: Vec<Value>) -> Result<Value> {
        self.session.touch();
        self.client.request(method, Value::Array(args)).await
    }
    pub(super) async fn lines(&self, buffer: u64) -> Result<Vec<String>> {
        let result = self
            .call(
                "nvim_buf_get_lines",
                vec![Value::from(buffer), 0.into(), (-1).into(), false.into()],
            )
            .await?;
        result
            .as_array()
            .context("buffer lines were not an array")?
            .iter()
            .map(|line| {
                line.as_str()
                    .map(str::to_owned)
                    .context("buffer line was not a string")
            })
            .collect()
    }
    pub(super) async fn changedtick(&self, buffer: u64) -> Result<u64> {
        self.call("nvim_buf_get_changedtick", vec![Value::from(buffer)])
            .await?
            .as_u64()
            .context("changedtick was not an integer")
    }
    pub(super) fn send(&self, message: ControlMsg) {
        let _ = self.controls.send(message);
    }
    pub(super) async fn is_current(&self, buffer: u64) -> bool {
        self.state
            .lock()
            .await
            .document
            .as_ref()
            .is_some_and(|d| d.buffer == buffer)
    }
    pub(super) async fn current_tick(&self) -> u64 {
        self.state
            .lock()
            .await
            .document
            .as_ref()
            .map_or(0, |d| d.changedtick)
    }
    pub(super) async fn resync(&self, reason: &str) {
        self.resync_with_tick(self.current_tick().await).await;
        self.send(ControlMsg::Error {
            message: reason.into(),
        });
    }
    pub(super) async fn resync_with_tick(&self, changedtick: u64) {
        self.send(ControlMsg::ResyncRequired {
            changedtick,
            reason: "buffer changed outside the incremental stream".into(),
        });
    }
}

fn value_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        crate::nvim_ui::rpc::value::ext_or_int(value)
            .ok()
            .and_then(|number| u64::try_from(number).ok())
    })
}
