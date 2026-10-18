//! The JavaScript sandbox behind `execute`.
//!
//! QuickJS with no host bindings except what [`ToolHost`] backs: `tools.*`, `console.*`,
//! and a capped `setTimeout`. A fresh runtime is built per run, so nothing a script does
//! survives it, and the limits below are enforced by the engine rather than by trust.

mod engine;
mod error;
mod host;
mod limits;
mod output;
mod state;

pub use engine::run;
pub use host::{Attachment, ToolHost, ToolOutput};
pub use limits::{Cancel, Limits};
pub use output::{truncate, Capped};
pub use state::{CallRecord, LogLine, Report};

/// What a run ended in.
#[derive(Clone, Debug, PartialEq)]
pub enum Finish {
    /// The script returned; this is its value, already printed (strings raw, else JSON).
    Value(String),
    /// The script threw, or did not compile.
    Failed(error::Failure),
    TimedOut,
    Cancelled,
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod sandbox_tests;

#[cfg(test)]
mod language_tests;
