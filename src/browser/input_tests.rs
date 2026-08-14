use super::*;

#[test]
fn plain_character_carries_text() {
    let key = Key::parse("a").expect("`a` is a key");
    assert_eq!(key.text.as_deref(), Some("a"));
    assert_eq!(key.modifiers, 0);
    assert_eq!(key.code, "KeyA");
}

#[test]
fn modified_character_carries_no_text() {
    // Ctrl+A that also inserts an "a" would replace the selection it just made.
    let key = Key::parse("Control+a").expect("ctrl+a is a chord");
    assert_eq!(key.text, None);
    assert_eq!(key.modifiers, MOD_CTRL);
}

#[test]
fn named_keys_resolve_case_insensitively() {
    for name in ["Enter", "enter", "ENTER"] {
        let key = Key::parse(name).expect("enter is a key");
        assert_eq!(key.virtual_code, 13);
        assert_eq!(key.key, "Enter");
    }
}

#[test]
fn escape_produces_an_event_but_no_character() {
    let key = Key::parse("Escape").expect("escape is a key");
    assert_eq!(key.text, None);
    assert_eq!(key.virtual_code, 27);
}

#[test]
fn modifiers_combine() {
    let key = Key::parse("Control+Shift+ArrowLeft").expect("chord parses");
    assert_eq!(key.modifiers, MOD_CTRL | MOD_SHIFT);
    assert_eq!(key.key, "ArrowLeft");
}

#[test]
fn unknown_names_are_rejected_rather_than_guessed() {
    assert!(Key::parse("Fnord").is_err());
    assert!(Key::parse("Hyper+a").is_err());
    assert!(Key::parse("").is_err());
}

#[test]
fn mouse_phases_map_to_cdp_names() {
    assert_eq!(MouseKind::Move.cdp_type(), "mouseMoved");
    assert_eq!(MouseKind::Down.cdp_type(), "mousePressed");
    assert_eq!(MouseKind::Up.cdp_type(), "mouseReleased");
    // A move with a button held would drag; panes send discrete phases.
    assert_eq!(MouseKind::Move.button(), "none");
    assert_eq!(MouseKind::Down.click_count(), 1);
}

#[test]
fn resolver_errors_become_rust_errors() {
    let resolved = Resolved {
        error: Some("ref e9 is not on this page".into()),
        x: 0,
        y: 0,
        tag: String::new(),
        editable: false,
        select: false,
    };
    let error = resolved.into_target().expect_err("an error must not resolve");
    assert!(error.to_string().contains("not on this page"));
}

#[test]
fn resolved_coordinates_survive_conversion() {
    let resolved = Resolved {
        error: None,
        x: 40,
        y: 90,
        tag: "input".into(),
        editable: true,
        select: false,
    };
    let target = resolved.into_target().expect("no error means a target");
    assert_eq!((target.x, target.y), (40, 90));
    assert!(target.editable);
}
