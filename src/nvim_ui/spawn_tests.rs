use super::*;

#[test]
fn socket_path_is_keyed_and_uses_the_ui_prefix() {
    let one = socket_path(&SessionKey::new(1, "one"));
    let two = socket_path(&SessionKey::new(1, "two"));
    assert_ne!(one, two);
    assert!(one
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("nvim-ui-")));
}
