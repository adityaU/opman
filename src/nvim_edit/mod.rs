//! UNIT V1: the Neovim edit-engine protocol.

mod actions;
mod columns;
mod commands;
mod engine;
mod events;
mod input;
mod notifications;
mod ops;
mod snapshot;
mod state;
mod sync;

pub(crate) use engine::EditEngine;
