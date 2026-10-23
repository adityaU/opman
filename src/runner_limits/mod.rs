//! Moving a session to another runner when its plan runs low.
//!
//! [`settings`] is the user's choice (policy, threshold, ordered runner + model pairs),
//! stored by the server so it applies with no browser open. [`choose`] holds the rules
//! for where to go. [`watch`] does the moving: it keeps a reading of every account, turns
//! a send to a low runner into a handoff, and continues a turn that failed on a limit.

pub mod choose;
pub mod model_sets;
mod model_state;
pub mod settings;
pub mod watch;

pub use settings::LimitSettings;
pub use watch::{on_limit_hit, spawn_poller};
