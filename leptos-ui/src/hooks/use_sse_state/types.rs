//! SSE state types — enums and constants shared across the SSE state module.

/// Number of messages to load per page.
pub const MESSAGE_PAGE_SIZE: usize = 50;

/// SSE connection status.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
}

impl ConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Disconnected => "disconnected",
        }
    }
}

/// Session status.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SessionStatus {
    Idle,
    Busy,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
        }
    }
}
