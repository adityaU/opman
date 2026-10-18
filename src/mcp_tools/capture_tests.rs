use super::capture::HeadTail;
use super::limits::{ByteLimit, Timeout};

#[test]
fn short_input_is_kept_whole() {
    let mut kept = HeadTail::new(ByteLimit::new(100));
    kept.push(b"hello ");
    kept.push(b"world");
    let done = kept.finish();
    assert_eq!(done.text, "hello world");
    assert_eq!(done.omitted, 0);
}

#[test]
fn long_input_keeps_start_and_end_and_counts_the_rest() {
    let mut kept = HeadTail::new(ByteLimit::new(30));
    for chunk in 0..100 {
        kept.push(format!("{chunk:03};").as_bytes());
    }
    let done = kept.finish();
    assert!(done.text.starts_with("000;001;"), "{}", done.text);
    assert!(done.text.ends_with("098;099;"), "{}", done.text);
    assert!(done.text.contains("bytes omitted"));
    assert_eq!(done.omitted, 400 - 30);
}

#[test]
fn cut_points_never_split_a_character() {
    let mut kept = HeadTail::new(ByteLimit::new(10));
    kept.push("\u{e9}".repeat(50).as_bytes());
    let done = kept.finish();
    assert!(!done.text.contains('\u{fffd}'), "{}", done.text);
}

#[test]
fn requested_limits_are_clamped() {
    let default = ByteLimit::new(10);
    let max = ByteLimit::new(50);
    assert_eq!(ByteLimit::requested(None, default, max), default);
    assert_eq!(ByteLimit::requested(Some(0), default, max).get(), 1);
    assert_eq!(ByteLimit::requested(Some(9999), default, max), max);
    let t = Timeout::requested_ms(Some(999_999), Timeout::from_secs(1), Timeout::from_secs(5));
    assert_eq!(t.get().as_secs(), 5);
}
