use std::path::Path;
use std::str::FromStr;

use super::super::types::{SocketRequest, SocketResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserCommand {
    Write,
    WriteAll,
    Quit,
    ForceQuit,
    BufferDelete,
    NoHighlight,
    EditReload,
    Undo,
    Redo,
}

impl FromStr for BrowserCommand {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "w" => Ok(Self::Write),
            "wa" => Ok(Self::WriteAll),
            "q" => Ok(Self::Quit),
            "q!" => Ok(Self::ForceQuit),
            "bd" => Ok(Self::BufferDelete),
            "noh" => Ok(Self::NoHighlight),
            "e!" => Ok(Self::EditReload),
            "undo" => Ok(Self::Undo),
            "redo" => Ok(Self::Redo),
            _ => Err(()),
        }
    }
}

impl BrowserCommand {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Write => "w",
            Self::WriteAll => "wa",
            Self::Quit => "q",
            Self::ForceQuit => "q!",
            Self::BufferDelete => "bd",
            Self::NoHighlight => "noh",
            Self::EditReload => "e!",
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }
}

pub(super) fn handle_browser(socket: &Path, command: BrowserCommand) -> SocketResponse {
    match crate::nvim_rpc::nvim_command(socket, command.as_str()) {
        Ok(()) => SocketResponse::ok_text(format!("Command executed: {}", command.as_str())),
        Err(error) => SocketResponse::err(format!("Neovim command failed: {error}")),
    }
}

pub(super) fn handle(socket: &Path, request: &SocketRequest) -> SocketResponse {
    let Some(command) = request.command.as_deref() else {
        return SocketResponse::err("Missing 'command' for nvim_command".into());
    };
    match crate::nvim_rpc::nvim_command(socket, command) {
        Ok(()) => SocketResponse::ok_text(format!("Command executed: {command}")),
        Err(error) => SocketResponse::err(format!("Neovim command failed: {error}")),
    }
}
