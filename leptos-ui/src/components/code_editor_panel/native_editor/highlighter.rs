//! Syntax highlighting via syntect — maps file extensions to highlighted line spans.

use std::cell::RefCell;
use std::rc::Rc;

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// A single styled text span within a line.
#[derive(Clone, Debug)]
pub struct StyledSpan {
    pub text: String,
    pub fg: String, // CSS color, e.g. "rgb(200,180,240)"
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// Shared syntax highlighting state.
#[derive(Clone)]
pub struct SyntaxHighlighter {
    inner: Rc<RefCell<HighlighterInner>>,
}

struct HighlighterInner {
    ss: SyntaxSet,
    theme: Theme,
    /// Index of the current syntax in the SyntaxSet.
    syntax_idx: usize,
    /// Cached highlighter state, reset on content change.
    hl: Option<HighlightLines<'static>>,
    /// Line cache: highlighted spans per line index.
    cache: Vec<Option<Vec<StyledSpan>>>,
}

impl SyntaxHighlighter {
    /// Create a highlighter for the given file extension (e.g. "rs", "ts").
    pub fn new(extension: &str) -> Self {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = ts.themes["base16-ocean.dark"].clone();
        let syntax_idx = ss
            .find_syntax_by_extension(extension)
            .map(|s| {
                // Find the index of this syntax in the set
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
                theme,
                syntax_idx,
                hl: None,
                cache: Vec::new(),
            })),
        }
    }

    /// Highlight a single line, returning styled spans.
    /// Lines are 0-indexed. Caches results; call `invalidate` on content changes.
    pub fn highlight_line(&self, line_idx: usize, line_text: &str) -> Vec<StyledSpan> {
        let mut inner = self.inner.borrow_mut();

        // Extend cache if needed
        if line_idx < inner.cache.len() {
            if let Some(ref cached) = inner.cache[line_idx] {
                return cached.clone();
            }
        }

        // We need to re-highlight from the beginning up to this line if no HL state.
        // For simplicity, create a fresh highlighter each time we need to go past cache.
        // This is acceptable for files of moderate size; can be optimized later.
        let spans = highlight_line_fresh(&inner.ss, &inner.theme, inner.syntax_idx, line_text);

        // Store in cache
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
        inner.hl = None;
    }

    /// Get the theme's default background color as a CSS string.
    pub fn background_color(&self) -> String {
        let inner = self.inner.borrow();
        inner
            .theme
            .settings
            .background
            .map(|c| format!("rgb({},{},{})", c.r, c.g, c.b))
            .unwrap_or_else(|| "transparent".to_string())
    }

    /// Get the theme's default foreground color as a CSS string.
    pub fn foreground_color(&self) -> String {
        let inner = self.inner.borrow();
        inner
            .theme
            .settings
            .foreground
            .map(|c| format!("rgb({},{},{})", c.r, c.g, c.b))
            .unwrap_or_else(|| "rgb(224,224,224)".to_string())
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
        inner.hl = None;
    }
}

/// Highlight a single line with a fresh highlighter (stateless, for simplicity).
fn highlight_line_fresh(
    ss: &SyntaxSet,
    theme: &Theme,
    syntax_idx: usize,
    line_text: &str,
) -> Vec<StyledSpan> {
    let syntax = &ss.syntaxes()[syntax_idx];
    // SAFETY: We're constructing HighlightLines with references that live long enough
    // within this function call. The lifetime issue is that HighlightLines borrows Theme,
    // but we own it through Rc<RefCell<>> and hold the borrow for the duration.
    let mut hl = HighlightLines::new(syntax, theme);
    let text = if line_text.ends_with('\n') {
        line_text.to_string()
    } else {
        format!("{}\n", line_text)
    };
    let ranges = hl.highlight_line(&text, ss).unwrap_or_default();
    ranges
        .into_iter()
        .map(|(style, text)| style_to_span(style, text))
        .collect()
}

fn style_to_span(style: Style, text: &str) -> StyledSpan {
    let fg = format!(
        "rgb({},{},{})",
        style.foreground.r, style.foreground.g, style.foreground.b
    );
    StyledSpan {
        text: text.to_string(),
        fg,
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
    }
}
