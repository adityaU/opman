//! Borrowed notification delivery for the Neovim RPC reader.

/// Receives a server notification while its encoded bytes are still owned by
/// the reader's framer.
///
/// Both arguments are valid only for the duration of the call. Implementors
/// that need to retain data must decode or copy it before returning. Keeping
/// this callback synchronous is what lets the hot redraw path remain
/// zero-copy at the RPC boundary.
pub trait NotificationSink: Send + Sync + 'static {
    fn notify(&self, method: &str, params: &[u8]);
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod notify_tests;
