//! Web-specific PTY manager.
//!
//! Owns independent PTY instances for the web UI — completely separate from
//! the TUI's PTYs. Each web terminal panel (shell, neovim, gitui, opencode)
//! gets its own process.
//!
//! Raw PTY output bytes are captured into per-PTY ring buffers so that
//! xterm.js receives genuine VT100 escape sequences (not stripped text).

mod activity;
mod buffer;
mod commands;
mod handle;
mod manager;
mod spawn;

pub use activity::PtyActivity;
pub use handle::WebPtyHandle;
pub use manager::start_web_pty_manager;

#[cfg(test)]
pub(crate) use buffer::RawOutputBuffer;
#[cfg(test)]
pub(crate) use manager::pty_test_support;
