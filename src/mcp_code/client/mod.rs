//! The MCP client side: reaching the other servers from inside the sandbox.
//!
//! A [`Pool`] holds one lazily opened [`Connection`] per eligible server. Nothing is
//! started until a catalog or a call needs it, and a connection then lives for the rest
//! of the process, so a script that calls the same server a hundred times pays for one
//! launch.

mod conn;
mod frame;
mod host;
mod launch;
mod pool;
mod select;

#[cfg(test)]
pub use conn::Connection;
pub use host::PoolHost;
pub use pool::Pool;
#[cfg(test)]
pub use pool::{Entry, Opener};
pub use select::pool_from_registry;
