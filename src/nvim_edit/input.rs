//! Input forwarding.
//!
//! Every key goes to Neovim exactly as the browser encoded it. opman does not
//! coalesce prefixes, does not track operator-pending state, and does not read
//! the command line: interpreting Vim's grammar a second time only produces a
//! second, worse Vim. What comes back — mode, cursor, command line, messages —
//! is painted from Neovim's own `redraw` events.

use std::sync::Arc;

use anyhow::Result;
use rmpv::Value;

use super::engine::EditEngine;

impl EditEngine {
    pub(super) async fn input(self: &Arc<Self>, method: &str, args: Vec<Value>) -> Result<()> {
        // Input must not wait behind a state snapshot or another deferred
        // Neovim request.  In particular, nvim_exec_lua can remain pending
        // while Neovim waits for the second key of a prefix; serializing the
        // fast nvim_input call here would make that prefix impossible to
        // complete.
        self.call(method, args).await?;
        Ok(())
    }
}
