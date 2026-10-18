//! Bounded text: nothing the sandbox reports may grow without limit.

/// Cut `text` to at most `max` bytes on a character boundary.
/// Returns the kept text and whether anything was cut.
pub fn truncate(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Text that may have been shortened, with the original size so the cut is visible.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capped {
    pub text: String,
    pub truncated: bool,
    pub bytes: usize,
}

impl Capped {
    pub fn new(text: &str, max: usize) -> Self {
        let (kept, truncated) = truncate(text, max);
        Self {
            text: kept.to_string(),
            truncated,
            bytes: text.len(),
        }
    }

    /// A stand-in for text dropped because a total budget ran out.
    pub fn omitted(bytes: usize) -> Self {
        Self {
            text: String::new(),
            truncated: true,
            bytes,
        }
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod output_tests;
