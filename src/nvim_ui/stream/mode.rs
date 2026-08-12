//! Neovim mode codes and their semantic short names.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NvimMode {
    #[serde(rename = "n")]
    Normal,
    #[serde(rename = "no")]
    OperatorPending,
    #[serde(rename = "nov")]
    OperatorPendingVisual,
    #[serde(rename = "noV")]
    OperatorPendingLine,
    #[serde(rename = "no\u{16}")]
    OperatorPendingBlock,
    #[serde(rename = "i")]
    Insert,
    #[serde(rename = "ic")]
    InsertComplete,
    #[serde(rename = "R")]
    Replace,
    #[serde(rename = "Rv")]
    ReplaceVirtual,
    #[serde(rename = "v")]
    Visual,
    #[serde(rename = "V")]
    VisualLine,
    #[serde(rename = "\u{16}")]
    VisualBlock,
    #[serde(rename = "s")]
    Select,
    #[serde(rename = "S")]
    SelectLine,
    #[serde(rename = "c")]
    Command,
    #[serde(rename = "t")]
    Terminal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeShort {
    Normal,
    Insert,
    Replace,
    Visual,
    VisualLine,
    VisualBlock,
    OperatorPending,
    Select,
    SelectLine,
    Command,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownNvimMode;

impl std::fmt::Display for UnknownNvimMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Neovim returned an unsupported mode")
    }
}

impl std::error::Error for UnknownNvimMode {}

impl TryFrom<&str> for NvimMode {
    type Error = UnknownNvimMode;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "n" => Self::Normal,
            "no" => Self::OperatorPending,
            "nov" => Self::OperatorPendingVisual,
            "noV" => Self::OperatorPendingLine,
            "no^V" | "no\u{16}" => Self::OperatorPendingBlock,
            "i" => Self::Insert,
            "ic" => Self::InsertComplete,
            "R" => Self::Replace,
            "Rv" => Self::ReplaceVirtual,
            "v" => Self::Visual,
            "V" => Self::VisualLine,
            "^V" | "\u{16}" => Self::VisualBlock,
            "s" => Self::Select,
            "S" => Self::SelectLine,
            "c" => Self::Command,
            "t" => Self::Terminal,
            _ => return Err(UnknownNvimMode),
        })
    }
}

impl NvimMode {
    pub fn short(self) -> ModeShort {
        match self {
            Self::Normal => ModeShort::Normal,
            Self::OperatorPending
            | Self::OperatorPendingVisual
            | Self::OperatorPendingLine
            | Self::OperatorPendingBlock => ModeShort::OperatorPending,
            Self::Insert | Self::InsertComplete => ModeShort::Insert,
            Self::Replace | Self::ReplaceVirtual => ModeShort::Replace,
            Self::Visual => ModeShort::Visual,
            Self::VisualLine => ModeShort::VisualLine,
            Self::VisualBlock => ModeShort::VisualBlock,
            Self::Select => ModeShort::Select,
            Self::SelectLine => ModeShort::SelectLine,
            Self::Command => ModeShort::Command,
            Self::Terminal => ModeShort::Terminal,
        }
    }

    pub fn is_visual(self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine | Self::VisualBlock)
    }
}
