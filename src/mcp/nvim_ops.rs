use std::fmt;
use std::str::FromStr;

/// The complete, closed set of Neovim operations exposed by MCP and the web UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NvimOp {
    Open,
    Read,
    Command,
    Input,
    Buffers,
    Info,
    Diagnostics,
    Definition,
    References,
    Hover,
    Symbols,
    CodeActions,
    Eval,
    Grep,
    Diff,
    Write,
    EditAndSave,
    Undo,
    Rename,
    Format,
    Signature,
}

/// Security capability required by a Neovim operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Read,
    Edit,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidNvimOp;

impl NvimOp {
    pub const ALL: [Self; 21] = [
        Self::Open,
        Self::Read,
        Self::Command,
        Self::Input,
        Self::Buffers,
        Self::Info,
        Self::Diagnostics,
        Self::Definition,
        Self::References,
        Self::Hover,
        Self::Symbols,
        Self::CodeActions,
        Self::Eval,
        Self::Grep,
        Self::Diff,
        Self::Write,
        Self::EditAndSave,
        Self::Undo,
        Self::Rename,
        Self::Format,
        Self::Signature,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "nvim_open",
            Self::Read => "nvim_read",
            Self::Command => "nvim_command",
            Self::Input => "nvim_input",
            Self::Buffers => "nvim_buffers",
            Self::Info => "nvim_info",
            Self::Diagnostics => "nvim_diagnostics",
            Self::Definition => "nvim_definition",
            Self::References => "nvim_references",
            Self::Hover => "nvim_hover",
            Self::Symbols => "nvim_symbols",
            Self::CodeActions => "nvim_code_actions",
            Self::Eval => "nvim_eval",
            Self::Grep => "nvim_grep",
            Self::Diff => "nvim_diff",
            Self::Write => "nvim_write",
            Self::EditAndSave => "nvim_edit_and_save",
            Self::Undo => "nvim_undo",
            Self::Rename => "nvim_rename",
            Self::Format => "nvim_format",
            Self::Signature => "nvim_signature",
        }
    }

    pub const fn capability(&self) -> Capability {
        match self {
            Self::Command | Self::Eval => Capability::Execute,
            Self::Open
            | Self::Input
            | Self::Write
            | Self::EditAndSave
            | Self::Undo
            | Self::Rename
            | Self::Format => Capability::Edit,
            Self::Read
            | Self::Buffers
            | Self::Info
            | Self::Diagnostics
            | Self::Definition
            | Self::References
            | Self::Hover
            | Self::Symbols
            | Self::CodeActions
            | Self::Grep
            | Self::Diff
            | Self::Signature => Capability::Read,
        }
    }

    pub const fn needs_buffer(&self) -> bool {
        matches!(
            self,
            Self::Read
                | Self::Info
                | Self::Diagnostics
                | Self::Definition
                | Self::References
                | Self::Hover
                | Self::Symbols
                | Self::CodeActions
                | Self::Diff
                | Self::Write
                | Self::EditAndSave
                | Self::Undo
                | Self::Rename
                | Self::Format
                | Self::Signature
        )
    }
}

impl FromStr for NvimOp {
    type Err = InvalidNvimOp;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "nvim_open" => Ok(Self::Open),
            "nvim_read" => Ok(Self::Read),
            "nvim_command" => Ok(Self::Command),
            "nvim_input" => Ok(Self::Input),
            "nvim_buffers" => Ok(Self::Buffers),
            "nvim_info" => Ok(Self::Info),
            "nvim_diagnostics" => Ok(Self::Diagnostics),
            "nvim_definition" => Ok(Self::Definition),
            "nvim_references" => Ok(Self::References),
            "nvim_hover" => Ok(Self::Hover),
            "nvim_symbols" => Ok(Self::Symbols),
            "nvim_code_actions" => Ok(Self::CodeActions),
            "nvim_eval" => Ok(Self::Eval),
            "nvim_grep" => Ok(Self::Grep),
            "nvim_diff" => Ok(Self::Diff),
            "nvim_write" => Ok(Self::Write),
            "nvim_edit_and_save" => Ok(Self::EditAndSave),
            "nvim_undo" => Ok(Self::Undo),
            "nvim_rename" => Ok(Self::Rename),
            "nvim_format" => Ok(Self::Format),
            "nvim_signature" => Ok(Self::Signature),
            _ => Err(InvalidNvimOp),
        }
    }
}

impl fmt::Display for NvimOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "nvim_ops_tests.rs"]
mod nvim_ops_tests;
