//! Web-specific PTY manager.
//!
//! Owns independent PTY instances for the web UI — completely separate from
//! the TUI's PTYs. Each web terminal (shell, neovim, gitui, opencode) gets its
//! own process, tagged with the project it was started in so the UI can offer
//! the shells belonging to one repo without listing every other repo's.
//!
//! A PTY lives for as long as its program does, not for as long as any pane
//! showing it: closing, zooming or re-laying-out a terminal detaches the view
//! and leaves the shell running. Only an explicit kill, the program exiting, or
//! the server shutting down ends one.
//!
//! Raw PTY output bytes are captured into per-PTY ring buffers so that
//! xterm.js receives genuine VT100 escape sequences (not stripped text).

mod activity;
mod buffer;
mod commands;
mod handle;
mod kind;
mod manager;
mod session;
mod spawn;

pub use handle::WebPtyHandle;
pub use kind::{PtyKind, PtyProgram, SpawnSpec};
pub use manager::start_web_pty_manager;
pub use session::PtySession;

#[cfg(test)]
pub(crate) use buffer::RawOutputBuffer;
#[cfg(test)]
pub(crate) use manager::pty_test_support;
