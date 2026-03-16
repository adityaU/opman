//! Lightweight syntax highlighter for chat code blocks.
//!
//! Uses syntect to parse code, then maps TextMate scopes to CSS token classes
//! that are already themed via `--color-syntax-*` vars in misc.css / messages-3.css.
//!
//! The output is an HTML string with `<span class="token keyword">...</span>` etc.
//! that the code block component can render via `inner_html`.

use std::cell::RefCell;
use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp, SyntaxSet};

thread_local! {
    static SYNTAX_SET: RefCell<SyntaxSet> = RefCell::new(SyntaxSet::load_defaults_newlines());
}

/// Map a language name (from fenced code block) to a syntect extension for lookup.
/// Returns a static extension string, or the original `lang` as fallback.
fn lang_to_extension<'a>(lang: &'a str) -> &'a str {
    let lower = lang.to_lowercase();
    match lower.as_str() {
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "python" | "py" => "py",
        "rust" | "rs" => "rs",
        "go" | "golang" => "go",
        "ruby" | "rb" => "rb",
        "java" => "java",
        "kotlin" | "kt" => "kt",
        "swift" => "swift",
        "c" => "c",
        "cpp" | "c++" | "cxx" => "cpp",
        "csharp" | "c#" | "cs" => "cs",
        "css" => "css",
        "scss" => "scss",
        "html" => "html",
        "xml" => "xml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "markdown" | "md" => "md",
        "bash" | "sh" | "shell" | "zsh" => "sh",
        "sql" => "sql",
        "lua" => "lua",
        "php" => "php",
        "zig" => "zig",
        "dockerfile" | "docker" => "Dockerfile",
        "makefile" | "make" => "Makefile",
        "diff" | "patch" => "diff",
        "elixir" | "ex" => "ex",
        "erlang" | "erl" => "erl",
        "haskell" | "hs" => "hs",
        "scala" => "scala",
        "r" => "r",
        "perl" | "pl" => "pl",
        "vim" => "vim",
        _ => lang,
    }
}

/// Classify a scope stack into one of the CSS `.token.*` classes.
/// Returns the class suffix: "keyword", "function", "string", etc.
fn classify_scope(scope: &ScopeStack) -> Option<&'static str> {
    // Walk scopes from most specific (last) to least specific (first).
    // TextMate scope naming convention: `keyword.control.rust`, `string.quoted.double`, etc.
    for s in scope.as_slice().iter().rev() {
        let atom_str = s.build_string();
        let a = atom_str.as_str();

        if a.starts_with("comment") {
            return Some("comment");
        }
        if a.starts_with("string") {
            return Some("string");
        }
        if a.starts_with("constant.numeric") {
            return Some("number");
        }
        if a.starts_with("constant.language") {
            return Some("boolean");
        }
        if a.starts_with("constant") {
            return Some("constant");
        }
        if a.starts_with("keyword") {
            return Some("keyword");
        }
        if a.starts_with("storage") {
            return Some("keyword");
        }
        if a.starts_with("entity.name.function") || a.starts_with("support.function") {
            return Some("function");
        }
        if a.starts_with("entity.name.type")
            || a.starts_with("entity.name.class")
            || a.starts_with("support.type")
            || a.starts_with("support.class")
        {
            return Some("class-name");
        }
        if a.starts_with("entity.name.tag") || a.starts_with("entity.name") {
            return Some("tag");
        }
        if a.starts_with("entity.other.attribute") {
            return Some("attr-name");
        }
        if a.starts_with("variable") {
            return Some("variable");
        }
        if a.starts_with("punctuation") {
            return Some("punctuation");
        }
        if a.starts_with("meta.function-call") {
            return Some("function");
        }
    }
    None
}

/// HTML-escape a string for safe embedding in innerHTML.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Highlight code and return an HTML string with `<span class="token ...">` markup.
///
/// If the language is unknown or highlighting fails, returns HTML-escaped plain text.
pub fn highlight_code(code: &str, language: &str) -> String {
    if language.is_empty() || language == "text" || language == "plaintext" {
        return html_escape(code);
    }

    let ext = lang_to_extension(language);

    SYNTAX_SET.with(|ss_cell| {
        let ss = ss_cell.borrow();
        let syntax = ss
            .find_syntax_by_extension(ext)
            .or_else(|| ss.find_syntax_by_name(language))
            .or_else(|| ss.find_syntax_by_name(&language.to_lowercase()));

        let syntax = match syntax {
            Some(s) => s,
            None => return html_escape(code),
        };

        let mut parse_state = ParseState::new(syntax);
        let mut scope_stack = ScopeStack::new();
        let mut html = String::with_capacity(code.len() * 2);

        for line in code.lines() {
            let line_with_nl = format!("{}\n", line);
            let ops = match parse_state.parse_line(&line_with_nl, &ss) {
                Ok(ops) => ops,
                Err(_) => {
                    html.push_str(&html_escape(line));
                    html.push('\n');
                    continue;
                }
            };

            let mut cur_pos = 0;

            for (byte_offset, op) in ops {
                let offset = byte_offset.min(line.len());
                // Emit text from cur_pos to this offset with current scope classification
                if offset > cur_pos {
                    let text = &line_with_nl[cur_pos..offset];
                    emit_span(&scope_stack, text, &mut html);
                }
                cur_pos = offset;
                let _ = scope_stack.apply(&op);
            }

            // Emit remaining text on the line (up to line.len(), not including the \n we added)
            if cur_pos < line.len() {
                let text = &line[cur_pos..];
                emit_span(&scope_stack, text, &mut html);
            }
            html.push('\n');
        }

        // Remove trailing newline if original code didn't end with one
        if !code.ends_with('\n') && html.ends_with('\n') {
            html.pop();
        }

        html
    })
}

/// Emit a text span with the appropriate CSS class, or plain escaped text.
fn emit_span(scope_stack: &ScopeStack, text: &str, html: &mut String) {
    if text.is_empty() {
        return;
    }
    let escaped = html_escape(text);
    match classify_scope(scope_stack) {
        Some(class) => {
            html.push_str("<span class=\"token ");
            html.push_str(class);
            html.push_str("\">");
            html.push_str(&escaped);
            html.push_str("</span>");
        }
        None => {
            html.push_str(&escaped);
        }
    }
}
