// ─── Tool definitions: Editing, Dev Flow, LSP Refactoring + assembler ────────

use super::tools_defs::{file_buffer_tool_defs, lsp_tool_defs};

/// Dev flow tool definitions (eval, grep).
fn devflow_tool_defs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "neovim_eval",
            "description": "Execute arbitrary Lua code inside the embedded Neovim instance and return the result. This is a powerful escape hatch for any Neovim operation not covered by the other tools. The code should use `return` to produce output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Lua code to execute in Neovim. Use `return` to produce output. Has full access to `vim.*` APIs."
                    }
                },
                "required": ["code"]
            }
        }),
        serde_json::json!({
            "name": "neovim_grep",
            "description": "Search across project files using Neovim's vimgrep. Returns matching file paths, line numbers, and matched text. Useful for finding usages, patterns, or text across the codebase.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The search pattern (Vim regex syntax)."
                    },
                    "glob": {
                        "type": "string",
                        "description": "File glob pattern to limit the search scope. Defaults to \"**/*\". Examples: \"**/*.rs\", \"src/**/*.ts\", \"*.py\"."
                    }
                },
                "required": ["pattern"]
            }
        }),
    ]
}

/// Editing tool definitions (edit_and_save, undo).
fn editing_tool_defs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "neovim_edit_and_save",
            "description": "Replace a range of lines in a Neovim buffer with new text and save the file to disk. This is the primary way to modify file content. Lines are 1-indexed and inclusive. file_path is required — use the buffer list to find open buffers. For multiple edits, pass an \"edits\" array instead of the single-edit parameters — line numbers are automatically adjusted after each edit so you can specify them based on the original file contents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to edit. Required for single edit, ignored when edits array is provided."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "First line to replace (1-indexed, inclusive)."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "Last line to replace (1-indexed, inclusive)."
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The replacement text. Use newlines (\\n) to separate multiple lines. Pass an empty string to delete the specified lines."
                    },
                    "edits": {
                        "type": "array",
                        "description": "Array of edits to apply as a batch. When provided, the single-edit parameters above are ignored. Line numbers should reference the original file — adjustments are computed automatically.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": {
                                    "type": "string",
                                    "description": "Absolute path of the file to edit."
                                },
                                "start_line": {
                                    "type": "number",
                                    "description": "First line to replace (1-indexed, inclusive)."
                                },
                                "end_line": {
                                    "type": "number",
                                    "description": "Last line to replace (1-indexed, inclusive)."
                                },
                                "new_text": {
                                    "type": "string",
                                    "description": "The replacement text."
                                }
                            },
                            "required": ["file_path", "start_line", "end_line", "new_text"]
                        }
                    }
                },
                "required": []
            }
        }),
        serde_json::json!({
            "name": "neovim_undo",
            "description": "Undo or redo changes in a Neovim buffer. Positive count undoes that many changes, negative count redoes. file_path is required — use the buffer list to find open buffers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to undo in. Required."
                    },
                    "count": {
                        "type": "number",
                        "description": "Number of changes to undo (positive) or redo (negative). Defaults to 1 undo."
                    }
                },
                "required": ["file_path"]
            }
        }),
    ]
}

/// LSP refactoring tool definitions (rename, format, signature).
fn refactoring_tool_defs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "neovim_rename",
            "description": "Rename a symbol across the project using the LSP. If file_path is provided, uses that file's buffer context; otherwise uses the current buffer. Requires an LSP server with rename support.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "new_name": {
                        "type": "string",
                        "description": "The new name for the symbol."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                },
                "required": ["new_name"]
            }
        }),
        serde_json::json!({
            "name": "neovim_format",
            "description": "Format a buffer using the LSP formatter (e.g., rustfmt, prettier, black). If file_path is provided, formats that file's buffer; otherwise formats the current buffer. Requires an LSP server with formatting support.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to format. If omitted, formats the current buffer."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_signature",
            "description": "Get function signature help at the specified position from the LSP. Shows parameter names, types, and documentation for function calls. If file_path is provided, uses that file's buffer context; otherwise uses the current buffer. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the call site. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the call site. Defaults to current cursor column."
                    }
                }
            }
        }),
    ]
}

/// Assemble all tool definitions into the complete list.
pub(super) fn tool_definitions() -> serde_json::Value {
    let mut tools = Vec::new();
    tools.extend(file_buffer_tool_defs());
    tools.extend(lsp_tool_defs());
    tools.extend(devflow_tool_defs());
    tools.extend(editing_tool_defs());
    tools.extend(refactoring_tool_defs());
    serde_json::Value::Array(tools)
}
