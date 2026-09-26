//! Runners that start their server on first use.

mod context;
mod runner;

pub use context::{LazyContext, LazyStart};
pub use runner::LazyRunner;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod readiness_tests;
