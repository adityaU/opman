//! Syntax highlighting via syntect — maps file extensions to CSS token classes.
//!
//! Instead of emitting inline RGB colors (from a syntect theme), we classify
//! TextMate scopes into `.token.*` CSS classes that are themed via
//! `--color-syntax-*` variables.  This keeps the editor visually consistent
//! with chat code blocks and both glassy/flat themes.

use std::cell::RefCell;
use std::rc::Rc;

use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

/// A single styled text span within a line.
#[derive(Clone, Debug)]
pub struct StyledSpan {
    pub text: String,
    /// CSS token class suffix (e.g. "keyword", "string"), or empty for plain text.
    pub token_class: &'static str,
    pub bold: bool,
    pub italic: bool,
}

/// Shared syntax highlighting state.
#[derive(Clone)]
pub struct SyntaxHighlighter {
    inner: Rc<RefCell<HighlighterInner>>,
}

struct HighlighterInner {
    ss: SyntaxSet,
    /// Index of the current syntax in the SyntaxSet.
    syntax_idx: usize,
    /// Line cache: highlighted spans per line index.
    cache: Vec<Option<Vec<StyledSpan>>>,
}

impl SyntaxHighlighter {
    /// Create a highlighter for the given file extension (e.g. "rs", "ts").
    pub fn new(extension: &str) -> Self {
        let ss = SyntaxSet::load_defaults_newlines();
        let syntax_idx = ss
            .find_syntax_by_extension(extension)
            .map(|s| {
                ss.syntaxes()
                    .iter()
                    .position(|x| x.name == s.name)
                    .unwrap_or(0)
            })
            .unwrap_or_else(|| {
                let plain = ss.find_syntax_plain_text();
                ss.syntaxes()
                    .iter()
                    .position(|x| x.name == plain.name)
                    .unwrap_or(0)
            });

        Self {
            inner: Rc::new(RefCell::new(HighlighterInner {
                ss,
                syntax_idx,
                cache: Vec::new(),
            })),
        }
    }

    /// Highlight a single line, returning styled spans.
    /// Lines are 0-indexed. Caches results; call `invalidate` on content changes.
    pub fn highlight_line(&self, line_idx: usize, line_text: &str) -> Vec<StyledSpan> {
        let inner = self.inner.borrow();

        if line_idx < inner.cache.len() {
            if let Some(ref cached) = inner.cache[line_idx] {
                return cached.clone();
            }
        }
        drop(inner);

        let mut inner = self.inner.borrow_mut();
        let spans = highlight_line_classify(&inner.ss, inner.syntax_idx, line_text);

        while inner.cache.len() <= line_idx {
            inner.cache.push(None);
        }
        inner.cache[line_idx] = Some(spans.clone());
        spans
    }

    /// Invalidate cached highlighting (call after any buffer edit).
    pub fn invalidate(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.cache.clear();
    }

    /// Change the syntax for a different file extension.
    pub fn set_extension(&self, extension: &str) {
        let mut inner = self.inner.borrow_mut();
        let idx = inner
            .ss
            .find_syntax_by_extension(extension)
            .map(|s| {
                inner
                    .ss
                    .syntaxes()
                    .iter()
                    .position(|x| x.name == s.name)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        inner.syntax_idx = idx;
        inner.cache.clear();
    }
}

/// Highlight a single line by parsing with syntect and classifying scopes.
fn highlight_line_classify(ss: &SyntaxSet, syntax_idx: usize, line_text: &str) -> Vec<StyledSpan> {
    let syntax = &ss.syntaxes()[syntax_idx];
    let mut parse_state = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();

    let text = if line_text.ends_with('\n') {
        line_text.to_string()
    } else {
        format!("{}\n", line_text)
    };

    let ops = match parse_state.parse_line(&text, ss) {
        Ok(ops) => ops,
        Err(_) => {
            return vec![StyledSpan {
                text: line_text.to_string(),
                token_class: "",
                bold: false,
                italic: false,
            }];
        }
    };

    let mut spans = Vec::new();
    let mut cur_pos = 0;
    let content_len = line_text.len();

    for (byte_offset, op) in ops {
        let offset = byte_offset.min(content_len);
        if offset > cur_pos {
            let chunk = &text[cur_pos..offset];
            let cls = classify_scope(&scope_stack);
            let is_italic = is_scope_italic(&scope_stack);
            spans.push(StyledSpan {
                text: chunk.to_string(),
                token_class: cls,
                bold: false,
                italic: is_italic,
            });
        }
        cur_pos = offset;
        let _ = scope_stack.apply(&op);
    }

    // Remaining text on the line (up to content_len, not the trailing \n)
    if cur_pos < content_len {
        let chunk = &line_text[cur_pos..];
        let cls = classify_scope(&scope_stack);
        spans.push(StyledSpan {
            text: chunk.to_string(),
            token_class: cls,
            bold: false,
            italic: false,
        });
    }

    spans
}

/// Classify a scope stack into a CSS `.token.*` class suffix.
/// Mirrors `syntax_highlight::classify_scope` for consistency.
fn classify_scope(scope: &ScopeStack) -> &'static str {
    for s in scope.as_slice().iter().rev() {
        let atom_str = s.build_string();
        let a = atom_str.as_str();

        if a.starts_with("comment") {
            return "comment";
        }
        if a.starts_with("string") {
            return "string";
        }
        if a.starts_with("constant.numeric") {
            return "number";
        }
        if a.starts_with("constant.language") {
            return "boolean";
        }
        if a.starts_with("constant") {
            return "constant";
        }
        if a.starts_with("keyword") || a.starts_with("storage") {
            return "keyword";
        }
        if a.starts_with("entity.name.function") || a.starts_with("support.function") {
            return "function";
        }
        if a.starts_with("entity.name.type")
            || a.starts_with("entity.name.class")
            || a.starts_with("support.type")
            || a.starts_with("support.class")
        {
            return "class-name";
        }
        if a.starts_with("entity.name.tag") || a.starts_with("entity.name") {
            return "tag";
        }
        if a.starts_with("entity.other.attribute") {
            return "attr-name";
        }
        if a.starts_with("variable") {
            return "variable";
        }
        if a.starts_with("punctuation") {
            return "punctuation";
        }
        if a.starts_with("meta.function-call") {
            return "function";
        }
    }
    ""
}

/// Check if the scope is a comment (should render italic).
fn is_scope_italic(scope: &ScopeStack) -> bool {
    for s in scope.as_slice().iter().rev() {
        let atom_str = s.build_string();
        if atom_str.starts_with("comment") {
            return true;
        }
    }
    false
}
