//! Keyboard input handling for the native editor.
//! Maps browser KeyboardEvents to floem-editor-core commands and text insertions.

use floem_editor_core::command::EditCommand;

/// Result of processing a keyboard event.
pub enum InputAction {
    /// Insert literal text at cursor.
    Insert(String),
    /// Execute an edit command.
    Command(EditCommand),
    /// No action (event not handled).
    None,
}

/// Map a browser keyboard event to an editor action.
/// `key` is KeyboardEvent.key, `ctrl` is ctrlKey||metaKey, `shift` is shiftKey.
pub fn map_key(key: &str, ctrl: bool, shift: bool) -> InputAction {
    if ctrl {
        return map_ctrl_key(key, shift);
    }

    match key {
        "Backspace" => InputAction::Command(EditCommand::DeleteBackward),
        "Delete" => InputAction::Command(EditCommand::DeleteForward),
        "Enter" => InputAction::Command(EditCommand::InsertNewLine),
        "Tab" if !shift => InputAction::Command(EditCommand::InsertTab),
        "Tab" if shift => InputAction::Command(EditCommand::OutdentLine),
        _ => {
            // Single printable character
            if key.len() == 1 || key.chars().count() == 1 {
                let c = key.chars().next().unwrap();
                if !c.is_control() {
                    return InputAction::Insert(key.to_string());
                }
            }
            InputAction::None
        }
    }
}

/// Handle Ctrl/Cmd key combinations.
fn map_ctrl_key(key: &str, shift: bool) -> InputAction {
    match key {
        "z" if !shift => InputAction::Command(EditCommand::Undo),
        "z" if shift => InputAction::Command(EditCommand::Redo),
        "y" => InputAction::Command(EditCommand::Redo),
        "c" => InputAction::Command(EditCommand::ClipboardCopy),
        "x" => InputAction::Command(EditCommand::ClipboardCut),
        "v" => InputAction::Command(EditCommand::ClipboardPaste),
        "a" => InputAction::None, // Let browser handle select-all
        "/" => InputAction::Command(EditCommand::ToggleLineComment),
        _ => InputAction::None,
    }
}
