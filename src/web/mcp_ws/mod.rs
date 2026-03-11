//! WebSocket-based MCP (Model Context Protocol) server for the web UI.
//!
//! Exposes web terminal and editor tools to AI agents via JSON-RPC 2.0
//! over WebSocket. This bridges the gap between the TUI's Unix socket
//! MCP server (`src/mcp.rs`) and the web UI, allowing AI agents running
//! in web-spawned sessions to control terminals and the CodeMirror editor.
//!
//! ## Protocol
//!
//! Standard MCP over JSON-RPC 2.0:
//! - `initialize` — handshake with capabilities
//! - `tools/list` — enumerate available tools
//! - `tools/call` — invoke a tool
//!
//! ## Tools
//!
//! Terminal tools (backed by `WebPtyHandle`):
//! - `web_terminal_read` — read output from a web PTY
//! - `web_terminal_run` — write a command to a web PTY
//! - `web_terminal_list` — list active web PTYs
//! - `web_terminal_new` — spawn a new web PTY shell
//! - `web_terminal_close` — kill a web PTY
//!
//! Editor tools (emit SSE events to frontend):
//! - `web_editor_open` — open a file in the CodeMirror editor
//! - `web_editor_read` — read a file from disk
//! - `web_editor_list` — list files in the project directory
//!
//! ## Authentication
//!
//! JWT token passed as `?token=<jwt>` query parameter (same as SSE endpoints).

mod editor;
mod handler;
mod protocol;
mod terminal;
mod tools;

pub use handler::websocket_handler;
