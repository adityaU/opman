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

    /// Invalidate cached highlighting from `from_line` onward.
    /// If `from_line` is `None`, clears the entire cache (e.g. file switch).
    pub fn invalidate_from(&self, from_line: Option<usize>) {
        let mut inner = self.inner.borrow_mut();
        match from_line {
            Some(line) if line < inner.cache.len() => {
                inner.cache.truncate(line);
            }
            Some(_) => {} // line beyond cache — nothing to invalidate
            None => inner.cache.clear(),
        }
    }

    /// Invalidate the entire cache (convenience alias for `invalidate_from(None)`).
    pub fn invalidate(&self) {
        self.invalidate_from(None);
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
            let (cls, is_italic) = classify_scope_and_italic(&scope_stack);
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
        let (cls, is_italic) = classify_scope_and_italic(&scope_stack);
        spans.push(StyledSpan {
            text: chunk.to_string(),
            token_class: cls,
            bold: false,
            italic: is_italic,
        });
    }

    spans
}

/// Classify a scope stack into a (CSS token class, italic) pair in a single pass.
/// Avoids calling `build_string()` twice per scope (once for class, once for italic).
fn classify_scope_and_italic(scope: &ScopeStack) -> (&'static str, bool) {
    for s in scope.as_slice().iter().rev() {
        let atom_str = s.build_string();
        let a = atom_str.as_str();

        if a.starts_with("comment") {
            return ("comment", true);
        }
        if a.starts_with("string") {
            return ("string", false);
        }
        if a.starts_with("constant.numeric") {
            return ("number", false);
        }
        if a.starts_with("constant.language") {
            return ("boolean", false);
        }
        if a.starts_with("constant") {
            return ("constant", false);
        }
        if a.starts_with("keyword") || a.starts_with("storage") {
            return ("keyword", false);
        }
        if a.starts_with("entity.name.function") || a.starts_with("support.function") {
            return ("function", false);
        }
        if a.starts_with("entity.name.type")
            || a.starts_with("entity.name.class")
            || a.starts_with("support.type")
            || a.starts_with("support.class")
        {
            return ("class-name", false);
        }
        if a.starts_with("entity.name.tag") || a.starts_with("entity.name") {
            return ("tag", false);
        }
        if a.starts_with("entity.other.attribute") {
            return ("attr-name", false);
        }
        if a.starts_with("variable") {
            return ("variable", false);
        }
        if a.starts_with("punctuation") {
            return ("punctuation", false);
        }
        if a.starts_with("meta.function-call") {
            return ("function", false);
        }
    }
    ("", false)
}
