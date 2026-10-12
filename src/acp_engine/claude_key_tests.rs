use serde_json::json;

use super::*;

const KEY: &str = "sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234567890ABCDEFGHIJ";

fn tail() -> &'static str {
    &KEY[KEY.len() - KEY_TAIL..]
}

/// The reported case: a key exported through launchctl, the user logged in with a Claude
/// subscription and never approved the key — interactive Claude ignores it, so must opman.
#[test]
fn an_unapproved_key_is_kept_from_a_subscription_user() {
    let config = json!({ "oauthAccount": { "emailAddress": "a@b.c" } });
    assert_eq!(decide(KEY, &config), KeyUse::Strip);
}

#[test]
fn an_approved_key_is_used() {
    let config = json!({
        "oauthAccount": { "emailAddress": "a@b.c" },
        "customApiKeyResponses": { "approved": [tail()], "rejected": [] },
    });
    assert_eq!(decide(KEY, &config), KeyUse::Keep);
}

#[test]
fn a_rejected_key_is_stripped_even_without_a_login() {
    let config = json!({ "customApiKeyResponses": { "approved": [], "rejected": [tail()] } });
    assert_eq!(decide(KEY, &config), KeyUse::Strip);
}

#[test]
fn with_no_login_and_no_answer_the_key_is_the_only_way_in() {
    assert_eq!(decide(KEY, &json!({})), KeyUse::Keep);
    assert_eq!(decide(KEY, &Value::Null), KeyUse::Keep);
}

#[test]
fn a_short_key_does_not_panic() {
    assert_eq!(decide("short", &json!({})), KeyUse::Keep);
}

#[test]
fn non_ascii_keys_are_sliced_on_character_boundaries() {
    assert_eq!(
        decide("ключ-ключ-ключ-ключ-ключ-ключ", &json!({})),
        KeyUse::Keep
    );
}
