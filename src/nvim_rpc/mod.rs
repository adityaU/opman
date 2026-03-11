mod buffer;
mod devtools;
mod edit;
mod lsp;
mod lsp_refactor;
mod lsp_symbols;
/// Minimal Neovim MessagePack-RPC client.
///
/// Connects to a neovim `--listen` Unix socket and sends synchronous
/// requests using the msgpack-rpc protocol (type 0 = request, type 1 = response).
///
/// Protocol format (msgpack array):
///   Request:  [0, msgid, method, params]
///   Response: [1, msgid, error, result]
mod transport;

// ─── Re-exports ─────────────────────────────────────────────────────────────
// Preserve the flat `crate::nvim_rpc::*` API that the rest of the codebase uses.

// transport (core RPC)
pub use transport::nvim_command;

// buffer operations
pub use buffer::{
    nvim_buf_diff, nvim_buf_get_lines, nvim_buf_get_name, nvim_buf_line_count, nvim_cursor_pos,
    nvim_find_or_load_buffer, nvim_list_bufs, nvim_open_file, nvim_undo, nvim_write,
};

// editing
pub use edit::{
    nvim_buf_multi_edit_and_save, nvim_buf_set_text_and_save, ResolvedEdit,
};

// LSP navigation & diagnostics
pub use lsp::{
    nvim_lsp_code_actions, nvim_lsp_definition, nvim_lsp_diagnostics, nvim_lsp_hover,
    nvim_lsp_references,
};

// LSP symbols
pub use lsp_symbols::nvim_lsp_symbols;

// LSP refactoring
pub use lsp_refactor::{nvim_lsp_format, nvim_lsp_rename, nvim_lsp_signature};

// dev-flow helpers
pub use devtools::{nvim_eval_lua, nvim_grep};
