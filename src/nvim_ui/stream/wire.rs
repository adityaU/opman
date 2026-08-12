//! Text WebSocket contract for the CodeMirror/Neovim edit engine.
//!
//! Frames are UTF-8 JSON text frames. `ClientMsg` is deliberately closed: the
//! browser names an operation by enum variant and can never provide an RPC
//! method or an Ex command string. Positions are zero-based and their
//! `column` is a CodeMirror-compatible UTF-16 offset. Neovim's byte columns
//! are converted at the edit-engine boundary.
//!
//! `ControlMsg` carries lifecycle, document, state, and message events. A
//! document event always carries the Neovim `changedtick` observed with that
//! event. `BufferChanged` is an incremental replacement of the half-open line
//! range `[first_line, last_line)`; `Attached` is the only wholesale snapshot.
//! An edit's `edit_id` is echoed as `origin` on its matching change event.

use serde::{Deserialize, Serialize};

pub use super::mode::{ModeShort, NvimMode, UnknownNvimMode};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TextPosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct VisualSelection {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "command")]
pub enum ExCommand {
    Write,
    WriteAll,
    Quit,
    ForceQuit,
    BufferDelete,
    NoHighlight,
    EditReload,
    Undo,
    Redo,
    Substitute {
        pattern: String,
        replacement: String,
        global: bool,
        ignore_case: bool,
    },
    Sort {
        reverse: bool,
        numeric: bool,
        unique: bool,
        ignore_case: bool,
    },
}

/// One entry of Neovim's buffer list, as the tabline surface needs it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct BufferEntry {
    pub name: String,
    pub modified: bool,
    pub current: bool,
}

/// Neovim's window and tab structure, refreshed after layout commands.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Layout {
    pub tabpages: u32,
    pub windows: u32,
    pub buffers: Vec<BufferEntry>,
}

/// JSON messages sent by the server on the text WebSocket channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
pub enum ControlMsg {
    Ready {},
    InputAck {},
    Attached {
        buffer: u64,
        path: String,
        changedtick: u64,
        lines: Vec<String>,
    },
    BufferChanged {
        buffer: u64,
        changedtick: u64,
        first_line: u32,
        last_line: u32,
        new_last_line: u32,
        lines: Vec<String>,
        origin: Option<String>,
    },
    BufferDetached {
        buffer: u64,
        changedtick: u64,
        reason: String,
    },
    ResyncRequired {
        changedtick: u64,
        reason: String,
    },
    State {
        changedtick: u64,
        cursor: TextPosition,
        mode: NvimMode,
        mode_short: ModeShort,
        visual: Option<VisualSelection>,
    },
    CommandOutput {
        changedtick: u64,
        output: String,
    },
    Message {
        changedtick: u64,
        kind: String,
        text: String,
    },
    /// Neovim's own command line, as it drew it. opman renders, never parses.
    Cmdline {
        visible: bool,
        first_char: String,
        content: String,
        position: u32,
    },
    /// The pattern whose matches are highlighted, or `None` once cleared.
    Search {
        pattern: Option<String>,
    },
    Layout {
        layout: Layout,
    },
    /// A Neovim mapping fired and named an editor action for opman to run.
    Action {
        name: String,
    },
    Error {
        message: String,
    },
    Exited {
        code: Option<i32>,
    },
    Superseded {},
    TooSlow {},
}

/// JSON messages accepted from the browser on the text WebSocket channel.
///
/// This is deliberately a closed enum. RPC method names must never cross the
/// browser boundary as data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
pub enum ClientMsg {
    Attach {
        path: String,
    },
    Edit {
        changedtick: u64,
        start: TextPosition,
        end: TextPosition,
        lines: Vec<String>,
        edit_id: String,
    },
    Input {
        keys: String,
    },
    /// Where the pointer put CodeMirror's caret, so Neovim follows a click.
    Cursor {
        position: TextPosition,
    },
    InputMouse {
        button: String,
        action: String,
        modifier: String,
        grid: i64,
        row: i64,
        col: i64,
    },
    Resize {
        rows: u16,
        cols: u16,
    },
    Paste {
        data: String,
    },
    Command {
        command: ExCommand,
    },
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod wire_tests;
