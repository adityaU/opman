mod bridge;
mod nvim_handler;
mod opencode_json;
mod server;
mod socket_client;
mod tool_defs;
mod tools;
mod types;

// Re-export all public items so `crate::mcp::*` continues to work.
pub use bridge::run_mcp_bridge;
pub use opencode_json::write_opencode_json;
pub use server::spawn_socket_server;
pub use types::{
    EditOp, NvimSocketRegistry, PendingSocketRequest, SocketRequest, SocketResponse, TabInfo,
    cleanup_socket, new_nvim_socket_registry, socket_path_for_project,
};
