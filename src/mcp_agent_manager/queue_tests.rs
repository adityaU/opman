//! Delivery mode parsing. Delivering itself needs a live registry and is covered by the
//! runner tests.

use super::*;

#[test]
fn immediate_and_its_alias_mean_steer_now() {
    assert_eq!(
        Delivery::parse(Some("immediate")).expect("known"),
        Some(Delivery::Immediate)
    );
    assert_eq!(
        Delivery::parse(Some("steer")).expect("known"),
        Some(Delivery::Immediate)
    );
}

#[test]
fn casing_and_padding_do_not_change_the_mode() {
    assert_eq!(
        Delivery::parse(Some("  QUEUED ")).expect("known"),
        Some(Delivery::Queued)
    );
}

#[test]
fn an_unknown_mode_names_both_legal_values() {
    let error = Delivery::parse(Some("eventually")).expect_err("only two modes exist");

    let text = format!("{error}");
    assert!(text.contains("immediate"), "{text}");
    assert!(text.contains("queued"), "{text}");
}
