//! Serializable types for the web API.
//!
//! These mirror the internal App/Session/PTY types but are decoupled for
//! independent evolution and to avoid leaking internal details.

mod activity;
mod autonomy;
mod computed;
mod computed_responses;
mod events;
mod files;
mod git;
mod memory;
mod missions;
mod presence;
mod requests;
mod sessions;
mod state;
mod system;
mod watchers;
mod workspaces;

pub use activity::*;
pub use autonomy::*;
pub use computed::*;
pub use computed_responses::*;
pub use events::*;
pub use files::*;
pub use git::*;
pub use memory::*;
pub use missions::*;
pub use presence::*;
pub use requests::*;
pub use sessions::*;
pub use state::*;
pub use system::*;
pub use watchers::*;
pub use workspaces::*;
