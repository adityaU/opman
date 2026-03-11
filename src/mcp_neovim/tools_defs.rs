// ─── Tool definitions: File & Buffer + LSP ──────────────────────────────────

/// Tool definitions for file & buffer operations.
pub(super) fn file_buffer_tool_defs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "neovim_open",
            "description": "Open a file in the embedded Neovim editor. Optionally jump to a specific line number. The file will be displayed in the Neovim pane of the opman TUI.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to open (absolute or relative to the project root)."
                    },
                    "line": {
                        "type": "number",
                        "description": "Optional line number to jump to (1-indexed). The view will be centered on this line."
                    }
                },
                "required": ["file_path"]
            }
        }),
        serde_json::json!({
            "name": "neovim_read",
            "description": "Read lines from a buffer in the embedded Neovim editor. Returns the text content of the specified line range with line numbers. If file_path is provided, reads from that file's buffer; otherwise reads from the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to read. If omitted, reads the current buffer."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "Start line (1-indexed, inclusive). Defaults to 1."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "End line (1-indexed, inclusive). Defaults to the last line of the buffer. Use -1 for the last line."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_command",
            "description": "Execute a Vim ex-command in the embedded Neovim editor. For example: \"set number\", \"w\", \"buffers\", \"%s/foo/bar/g\", etc. Do not include the leading colon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The Vim ex-command to execute (without the leading colon)."
                    }
                },
                "required": ["command"]
            }
        }),
        serde_json::json!({
            "name": "neovim_buffers",
            "description": "List all loaded buffers in the embedded Neovim editor. Returns buffer IDs and their associated file paths.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        serde_json::json!({
            "name": "neovim_info",
            "description": "Get information about the current state of the embedded Neovim editor: current buffer file path, cursor position (line, column), and total line count.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        serde_json::json!({
            "name": "neovim_write",
            "description": "Save a buffer (or all buffers) in the embedded Neovim editor. If file_path is provided, saves that file's buffer; otherwise saves the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to save. If omitted, saves the current buffer."
                    },
                    "all": {
                        "type": "boolean",
                        "description": "If true, save all modified buffers. If false or omitted, save only the targeted buffer."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_diff",
            "description": "Show unsaved changes in a Neovim buffer as a unified diff. Compares the buffer content against the file on disk. If file_path is provided, diffs that file's buffer; otherwise diffs the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to diff. If omitted, diffs the current buffer."
                    }
                }
            }
        }),
    ]
}

/// Tool definitions for LSP operations.
pub(super) fn lsp_tool_defs() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "neovim_diagnostics",
            "description": "Get LSP diagnostics (errors, warnings, hints) from Neovim. Returns structured diagnostic info including file, line, severity, message, and source. Requires an LSP server to be attached to the buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to get diagnostics for. If omitted, uses the current buffer."
                    },
                    "buf_only": {
                        "type": "boolean",
                        "description": "If true, return diagnostics only for the targeted buffer. If false or omitted, return diagnostics for all open buffers (project-wide)."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_definition",
            "description": "Go to the definition of the symbol at the specified position using the LSP. Jumps to the definition and returns the location(s). Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_references",
            "description": "Find all references to the symbol at the specified position using the LSP. Returns file paths, line numbers, and context for each reference. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_hover",
            "description": "Get hover/type information for the symbol at the specified position from the LSP. Returns type signatures, documentation, etc. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_symbols",
            "description": "Search for symbols using the LSP. Can search within a specific document or across the entire workspace. Returns symbol names, kinds, file locations, and line numbers. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to search for document symbols. If omitted, uses the current buffer. Ignored for workspace searches."
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter symbols. For workspace search, this filters by name. For document symbols, all symbols are returned (query is ignored)."
                    },
                    "workspace": {
                        "type": "boolean",
                        "description": "If true, search across the entire workspace. If false or omitted, search only the targeted document."
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "neovim_code_actions",
            "description": "List available LSP code actions at the current cursor position. Code actions include quick-fixes, refactors, and source actions. Returns action titles and kinds. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to get code actions for. If omitted, uses the current buffer."
                    }
                }
            }
        }),
    ]
}
