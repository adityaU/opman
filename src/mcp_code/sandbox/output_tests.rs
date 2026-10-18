use super::*;

#[test]
fn truncate_keeps_short_text() {
    assert_eq!(truncate("abc", 10), ("abc", false));
}

#[test]
fn truncate_cuts_on_a_character_boundary() {
    // Each of these is three bytes; a cut at 4 must back up to 3.
    let (kept, cut) = truncate("日本語", 4);
    assert_eq!(kept, "日");
    assert!(cut);
    assert_eq!(truncate("abc", 0), ("", true));
}

#[test]
fn capped_remembers_the_original_size() {
    let capped = Capped::new(&"x".repeat(100), 10);
    assert_eq!(capped.text.len(), 10);
    assert!(capped.truncated);
    assert_eq!(capped.bytes, 100);
    let whole = Capped::new("ok", 10);
    assert!(!whole.truncated);
    assert_eq!(whole.bytes, 2);
}

#[test]
fn omitted_is_empty_but_marked() {
    let gone = Capped::omitted(500);
    assert!(gone.text.is_empty() && gone.truncated);
    assert_eq!(gone.bytes, 500);
}
