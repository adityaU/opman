//! API client module — fetch wrappers with cookie auth.
//! Mirrors the TypeScript `api/` modules.

pub mod client;
pub mod pty;
pub mod files;
pub mod git;
pub mod editor;
pub mod watchers;
pub mod project;
pub mod session;
pub mod session_views;
pub mod missions;
pub mod intelligence;
pub mod workflows;
pub mod system;
pub mod health;

pub use client::*;
