pub mod attach;
pub mod error;
pub mod key;
pub mod pool;
pub mod reaper;
pub mod registry_guard;
pub mod rpc;
pub mod session;
pub mod spawn;
pub mod stream;

pub use key::{SessionKey, UiSize};
pub use pool::{NvimUiPool, MAX_SESSIONS};
pub use rpc::NvimClient;
pub use session::{NvimNotification, NvimSession};

#[cfg(test)]
#[path = "live_tests.rs"]
mod live;
