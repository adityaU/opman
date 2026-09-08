//! Runners that start their server on first use.

mod context;
mod runner;

pub use context::{LazyContext, LazyStart};
pub use runner::LazyRunner;

#[cfg(test)]
mod tests;
