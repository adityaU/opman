/// Return MCP tool definitions for tools/list.
pub fn mcp_tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "terminal_read",
            "description": "Read the terminal output from a terminal tab in the opman. Returns the full terminal buffer (scrollback + visible). Use last_n to limit to the most recent N lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based). Defaults to the active tab if not specified."
                    },
                    "last_n": {
                        "type": "number",
                        "description": "Return only the last N lines of the terminal output."
                    }
                }
            }
        },
        {
            "name": "terminal_run",
            "description": "Run a command in a terminal tab in the opman. The command is typed into the terminal and executed. Use this to run shell commands, scripts, or interact with running processes. If a command is already running on the tab, this will return an error — send Ctrl-C (\\x03) as the command to interrupt it first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run in the terminal. Send \"\\x03\" (Ctrl-C) to interrupt a running command."
                    },
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based). Defaults to the active tab if not specified."
                    },
                    "wait": {
                        "type": "boolean",
                        "description": "If true, wait for command output to settle and return the terminal screen content. If false (default), fire-and-forget — returns immediately, use terminal_read to check output later."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait for command to complete when wait=true (default: 30)."
                    }
                },
                "required": ["command", "tab"]
            }
        },
        {
            "name": "terminal_list",
            "description": "List all terminal tabs in the opman for the current project. Returns tab indices and which tab is currently active.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "terminal_new",
            "description": "Create a new terminal tab in the opman. Returns the index of the newly created tab.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Optional name for the new tab"
                    }
                }
            }
        },
        {
            "name": "terminal_close",
            "description": "Close a terminal tab in the opman.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based) to close. Defaults to the active tab if not specified."
                    }
                }
            }
        },
        {
            "name": "terminal_rename",
            "description": "Rename a terminal tab",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based) to rename"
                    },
                    "name": {
                        "type": "string",
                        "description": "New name for the tab"
                    }
                },
                "required": ["tab", "name"]
            }
        },
        {
            "name": "terminal_ephemeral_run",
            "description": "Run a command in a named ephemeral terminal tab. Creates a temporary tab (or reuses one with the same name), runs the command, waits for completion, returns the output, and closes the tab.\n\nUse a unique `name` for each independent task you want to run in PARALLEL (e.g. \"build\", \"test\", \"lint\"). Two calls with the same name cannot run concurrently — the second will be rejected. Use the SAME name for commands that must run SEQUENTIALLY on the same logical task.\n\nThis is the PREFERRED tool for one-shot commands — use this instead of terminal_run when you just need command output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run in the terminal."
                    },
                    "name": {
                        "type": "string",
                        "description": "A logical name for this task (e.g. \"build\", \"test\", \"lint\"). Use different names to run commands in parallel. Use the same name for sequential commands that belong to the same task — a second parallel call with the same name will be rejected."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait for command output to settle (default: 30). The tool polls until output stabilizes or timeout is reached."
                    }
                },
                "required": ["command", "name"]
            }
        }
    ])
}
