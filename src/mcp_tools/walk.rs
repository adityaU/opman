//! A project walker shared by `glob` and `grep`: honours `.gitignore`, skips `.git`, and
//! can be told to stop when the call is cancelled.

use super::output::ToolError;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Set when the caller goes away, so a long walk on a blocking thread can end early.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// A guard that sets this flag when dropped.
    pub fn on_drop(&self) -> CancelGuard {
        CancelGuard(self.clone())
    }
}

pub struct CancelGuard(Cancel);

impl Drop for CancelGuard {
    fn drop(&mut self) {
        (self.0).0.store(true, Ordering::Relaxed);
    }
}

/// Files under `root` (or `root` itself if it is a file), in name order. `filter` is a
/// gitignore-style glob such as `*.rs` or `src/**/*.ts`.
pub fn files(
    root: &Path,
    filter: Option<&str>,
    cancel: &Cancel,
) -> Result<impl Iterator<Item = ignore::DirEntry>, ToolError> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(|entry| entry.file_name() != ".git");
    if let Some(glob) = filter {
        let mut overrides = OverrideBuilder::new(root);
        overrides
            .add(glob)
            .map_err(|e| ToolError::new(format!("bad glob '{glob}': {e}")))?;
        let overrides = overrides
            .build()
            .map_err(|e| ToolError::new(format!("bad glob '{glob}': {e}")))?;
        builder.overrides(overrides);
    }
    let cancel = cancel.clone();
    Ok(builder
        .build()
        .take_while(move |_| !cancel.is_set())
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file())))
}
