//! SSE module — EventSource connections and message map for real-time updates.

pub mod connection;
pub mod message_map;

pub use connection::wire_sse;
