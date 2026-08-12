use super::*;

#[test]
fn size_rejects_zero_dimensions() {
    assert!(UiSize::new(0, 80).is_none());
    assert!(UiSize::new(24, 0).is_none());
}

#[test]
fn size_rejects_absurd_dimensions() {
    assert!(UiSize::new(MAX_UI_ROWS + 1, 80).is_none());
    assert!(UiSize::new(24, MAX_UI_COLS + 1).is_none());
}

#[test]
fn size_preserves_valid_dimensions() {
    let size = UiSize::new(40, 120).unwrap();
    assert_eq!(size.rows(), 40);
    assert_eq!(size.cols(), 120);
    assert_eq!(size.to_string(), "120x40");
}
