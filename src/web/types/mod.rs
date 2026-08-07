//! Serializable types for the web API.
//!
//! These mirror the internal App/Session/PTY types but are decoupled for
//! independent evolution and to avoid leaking internal details.

mod autonomy;
mod events;
mod files;
mod git;
pub mod health;
mod kanban;
mod kanban_pipeline;
mod memory;
mod presence;
mod requests;
mod search;
mod sessions;
mod state;
mod system;
mod watchers;

pub use autonomy::*;
pub use events::*;
pub use files::*;
pub use git::*;
pub use kanban::*;
pub use kanban_pipeline::*;
pub use memory::*;
pub use presence::*;
pub use requests::*;
pub use search::*;
pub use sessions::*;
pub use state::*;
pub use system::*;
pub use watchers::*;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
