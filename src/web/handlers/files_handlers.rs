//! File handlers, grouped by operation.
#[cfg(test)]
use super::super::auth::AuthUser;
#[cfg(test)]
use super::super::error::{WebError, WebResult};
#[cfg(test)]
use super::super::types::*;
#[cfg(test)]
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
#[path = "files_helpers.rs"]
mod helpers;
#[path = "files_mutate.rs"]
mod mutate;
#[path = "files_read.rs"]
mod read;
#[path = "files_search.rs"]
mod search;
#[path = "files_upload.rs"]
mod upload;
#[path = "files_write.rs"]
mod write;
pub(crate) use helpers::{detect_language, mime_from_extension};
pub use mutate::{delete_dir, delete_file, rename_entry};
pub use read::{browse_files, read_file, read_file_raw};
pub use search::search_files;
#[cfg(test)]
use search::{is_gitignored, load_gitignore, search_files_sync};
pub use upload::upload_files;
pub use write::{create_dir, create_file, write_file};

#[cfg(test)]
#[path = "files_handlers_browse_error_tests.rs"]
mod files_handlers_browse_error_tests;
#[cfg(test)]
#[path = "files_handlers_browse_tests.rs"]
mod files_handlers_browse_tests;
#[cfg(test)]
#[path = "files_handlers_edge_tests.rs"]
mod files_handlers_edge_tests;
#[cfg(test)]
#[path = "files_handlers_extra_tests.rs"]
mod files_handlers_extra_tests;
#[cfg(test)]
#[path = "files_handlers_helpers_tests.rs"]
mod files_handlers_helpers_tests;
#[cfg(test)]
#[path = "files_handlers_mutate_tests.rs"]
mod files_handlers_mutate_tests;
