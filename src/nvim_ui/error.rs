//! Errors produced by the Neovim UI process lifecycle.

use std::fmt;

#[derive(Debug)]
pub enum NvimUiError {
    Spawn(std::io::Error),
    MissingPipe(&'static str),
    InvalidSize,
    Rpc(anyhow::Error),
    Registry(std::io::Error),
}

impl fmt::Display for NvimUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to start Neovim: {error}"),
            Self::MissingPipe(pipe) => write!(f, "Neovim {pipe} was not piped"),
            Self::InvalidSize => f.write_str("invalid Neovim UI size"),
            Self::Rpc(error) => write!(f, "Neovim UI RPC failed: {error}"),
            Self::Registry(error) => write!(f, "Neovim socket registry failed: {error}"),
        }
    }
}

impl std::error::Error for NvimUiError {}

impl From<anyhow::Error> for NvimUiError {
    fn from(error: anyhow::Error) -> Self {
        Self::Rpc(error)
    }
}

pub type Result<T> = std::result::Result<T, NvimUiError>;
