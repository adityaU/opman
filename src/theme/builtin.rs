//! The themes compiled into opman (`opencode-themes/`).
//!
//! One copy serves both the theme picker's list and the colours of the active theme.
//! The two used to come from different places — the list from these files, the colours
//! only from OpenCode's folders on disk — so on a machine without OpenCode's source or
//! config (a fresh remote server, say) every theme in the list painted as the defaults.

use include_dir::{include_dir, Dir, File};
use serde_json::Value;

static THEMES: Dir = include_dir!("$CARGO_MANIFEST_DIR/opencode-themes");

/// Every built-in theme file.
pub fn files() -> impl Iterator<Item = &'static File<'static>> {
    THEMES.files()
}

/// The built-in theme called `name`, parsed. `None` when opman ships no such theme.
pub fn theme(name: &str) -> Option<Value> {
    let file = THEMES.get_file(format!("{name}.json"))?;
    serde_json::from_slice(file.contents()).ok()
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod builtin_tests;
