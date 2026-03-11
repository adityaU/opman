// ─── Response formatting + utilities ─────────────────────────────────────────

use crate::mcp::SocketResponse;

/// Map file extension to markdown language identifier for syntax highlighting.
pub fn ext_to_lang(path: &str) -> &str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "bash",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "sql" => "sql",
        "md" | "markdown" => "markdown",
        "zig" => "zig",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" | "R" => "r",
        "dart" => "dart",
        "vim" => "vim",
        "el" | "lisp" | "cl" => "lisp",
        "clj" | "cljs" => "clojure",
        "php" => "php",
        "pl" | "pm" => "perl",
        "scala" | "sc" => "scala",
        _ => "",
    }
}

/// Try to pretty-print a JSON string. Returns the original if it fails.
fn try_pretty_json(s: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| s.to_string()),
        Err(_) => s.to_string(),
    }
}

/// Convert a SocketResponse to MCP content format with proper markdown formatting.
///
/// Wraps output in markdown code fences with appropriate language identifiers
/// so that the AI client can render syntax-highlighted content.
pub(super) fn format_response(
    tool_name: &str,
    resp: &SocketResponse,
) -> anyhow::Result<serde_json::Value> {
    if !resp.ok {
        let error_msg = resp.error.as_deref().unwrap_or("Unknown error");
        return Ok(serde_json::json!([{
            "type": "text",
            "text": format!("Error: {}", error_msg)
        }]));
    }

    let text = resp.output.as_deref().unwrap_or("OK");

    let formatted = match tool_name {
        // ── File content: already wrapped in code fence by app.rs ───
        "neovim_read" => text.to_string(),

        // ── Diff output ──────────────────────────────────────────────
        "neovim_diff" => {
            if text.contains("No unsaved") || !text.contains("@@") {
                text.to_string()
            } else {
                format!("```diff\n{}\n```", text)
            }
        }

        // ── LSP hover: already markdown from LSP, pass through ──────
        "neovim_hover" => text.to_string(),

        // ── JSON responses: pretty-print and wrap ────────────────────
        "neovim_diagnostics"
        | "neovim_definition"
        | "neovim_references"
        | "neovim_symbols"
        | "neovim_code_actions"
        | "neovim_grep"
        | "neovim_rename"
        | "neovim_format"
        | "neovim_signature" => {
            let pretty = try_pretty_json(text);
            format!("```json\n{}\n```", pretty)
        }

        // ── Lua eval: wrap in code fence ────────────────────────────
        "neovim_eval" => format!("```\n{}\n```", text),

        // ── Plain text for everything else ──────────────────────────
        _ => text.to_string(),
    };

    Ok(serde_json::json!([{
        "type": "text",
        "text": formatted
    }]))
}
