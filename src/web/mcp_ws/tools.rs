//! MCP tool definitions (JSON schema for `tools/list`).

/// Tool definitions for the web MCP server.
pub(crate) fn web_mcp_tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "web_terminal_read",
            "description": "Read the terminal output from a web PTY terminal tab. Returns the raw terminal buffer content. Use `last_n` to limit to the most recent N lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID). Use web_terminal_list to discover available IDs."
                    },
                    "last_n": {
                        "type": "number",
                        "description": "Return only the last N lines of the terminal output."
                    }
                },
                "required": ["id"]
            }
        },
        {
            "name": "web_terminal_run",
            "description": "Run a command in a web PTY terminal tab. The command is typed into the terminal and Enter is pressed. If a command is already running, send \"\\x03\" (Ctrl-C) to interrupt it first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID). Use web_terminal_list to discover available IDs."
                    },
                    "command": {
                        "type": "string",
                        "description": "The command to run. Send \"\\x03\" (Ctrl-C) to interrupt a running command."
                    },
                    "wait": {
                        "type": "boolean",
                        "description": "If true, wait for command output to settle and return the terminal content. If false (default), fire-and-forget."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait when wait=true (default: 30)."
                    }
                },
                "required": ["id", "command"]
            }
        },
        {
            "name": "web_terminal_list",
            "description": "List all active web PTY terminal tabs. Returns an array of PTY IDs.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "web_terminal_new",
            "description": "Create a new web PTY terminal (shell). Returns the ID of the newly created PTY.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rows": {
                        "type": "number",
                        "description": "Terminal rows (default: 24)."
                    },
                    "cols": {
                        "type": "number",
                        "description": "Terminal columns (default: 80)."
                    }
                }
            }
        },
        {
            "name": "web_terminal_close",
            "description": "Close (kill) a web PTY terminal tab.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID) to close."
                    }
                },
                "required": ["id"]
            }
        },
        {
            "name": "web_editor_open",
            "description": "Open a file in the web UI's CodeMirror editor. Optionally navigate to a specific line.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number to navigate to (1-based)."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "web_editor_read",
            "description": "Read the content of a file from disk. Returns the file content as text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "Start line (1-based, inclusive). Omit to read from the beginning."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "End line (1-based, inclusive). Omit to read to the end."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "web_editor_list",
            "description": "List files in the project directory tree. Returns file paths relative to the project root.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Subdirectory to list (relative to project root). Defaults to project root."
                    },
                    "depth": {
                        "type": "number",
                        "description": "Maximum directory depth to traverse (default: 3)."
                    }
                }
            }
        }
    ])
}
