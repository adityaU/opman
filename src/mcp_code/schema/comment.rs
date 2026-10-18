//! Doc comments from schema descriptions, kept short and safe to embed.

/// Longest comment shown above a whole tool.
pub(super) const TOOL_LIMIT: usize = 400;
/// Longest comment shown above one field.
pub(super) const FIELD_LIMIT: usize = 160;

/// Collapse whitespace, cut to `limit` characters, and defuse `*/` so a description can
/// never close the comment early.
fn tidy(text: &str, limit: usize) -> String {
    let mut out = String::with_capacity(text.len().min(limit + 4));
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
        if out.chars().count() > limit {
            break;
        }
    }
    if out.chars().count() > limit {
        out = out.chars().take(limit).collect();
        out.push_str("...");
    }
    out.replace("*/", "* /")
}

/// A one-line `/** ... */` comment, or the empty string when there is nothing to say.
pub(super) fn line(text: &str, limit: usize) -> String {
    let body = tidy(text, limit);
    if body.is_empty() {
        return String::new();
    }
    format!("/** {body} */")
}

/// Same as [`line`]; named for the call sites that mean "above a tool".
pub(super) fn block(text: &str, limit: usize) -> String {
    line(text, limit)
}
