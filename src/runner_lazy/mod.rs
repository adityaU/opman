//! Runners that start their server on first use.

mod context;
mod disk;
mod runner;

pub use context::{LazyContext, LazyStart};
pub use runner::LazyRunner;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod readiness_tests;

#[cfg(test)]
#[path = "disk_tests.rs"]
mod disk_tests;
