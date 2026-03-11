/// Truncate a string to at most `max_bytes` bytes, ensuring the cut falls on a
/// UTF-8 char boundary.  Equivalent to the nightly `str::floor_char_boundary`.
#[inline]
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }
    let mut idx = max_bytes;
    // Walk backwards until we land on a char boundary.
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

/// Snap a byte index to the nearest char boundary at or below `idx`.
/// Returns 0 if `idx` is 0 or no valid boundary exists above 0.
#[inline]
pub fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let idx = idx.min(s.len());
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Safely split `s` at byte position `idx`, snapping to the nearest char
/// boundary at or below `idx`.  Returns `(before, after)`.
#[inline]
#[allow(dead_code)]
pub fn split_at_safe(s: &str, idx: usize) -> (&str, &str) {
    let i = floor_char_boundary(s, idx);
    (&s[..i], &s[i..])
}
