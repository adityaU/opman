mod bridge;
pub(crate) mod nvim_handler;
pub(crate) mod nvim_ops;
mod server;
mod socket_client;
mod tool_defs;
mod tools;
mod types;

// Re-export all public items so `crate::mcp::*` continues to work.
pub use bridge::run_mcp_bridge;
pub use nvim_ops::{Capability, NvimOp};
pub use server::spawn_socket_server;
pub use types::{
    cleanup_socket, new_nvim_socket_registry, socket_path_for_project, EditOp, NvimSocketRegistry,
    PendingSocketRequest, SocketRequest, SocketResponse, TabInfo,
};
