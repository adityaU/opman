//! Editor actions Neovim owns the keys for.
//!
//! The LSP shortcuts are real Neovim mappings, registered on the session, not a
//! browser-side interception. Pressing `<F12>` goes to Neovim like every other
//! key; Neovim decides it is a mapping and calls back with the action's name.
//! Rebinding it is then `vim.keymap.set`, the same as any other mapping.

use std::sync::Arc;

use rmpv::Value;

use super::engine::EditEngine;

const ACTIONS_LUA: &str = "local channel = ...
local function notify(name)
  return function() vim.rpcnotify(channel, 'opman_action', name) end
end
local modes = { 'n', 'i', 'v' }
vim.keymap.set(modes, '<F12>', notify('lsp.definition'), { silent = true })
vim.keymap.set(modes, '<S-F12>', notify('lsp.references'), { silent = true })
vim.keymap.set(modes, '<F2>', notify('lsp.rename'), { silent = true })
return true";

impl EditEngine {
    /// Teach this Neovim session the editor's own actions. Failure is not
    /// fatal: the buffer still works, the shortcuts just do nothing.
    pub(super) async fn install_actions(self: &Arc<Self>) {
        let Ok(info) = self.call("nvim_get_api_info", Vec::new()).await else {
            return;
        };
        let Some(channel) = info.as_array().and_then(|fields| fields.first()).cloned() else {
            return;
        };
        let _ = self
            .call(
                "nvim_exec_lua",
                vec![Value::from(ACTIONS_LUA), Value::Array(vec![channel])],
            )
            .await;
    }
}
