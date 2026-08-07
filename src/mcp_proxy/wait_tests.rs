//! The messages the held-open wait produces.

use super::*;

fn name() -> ServerName {
    ServerName::parse("linear").expect("valid")
}

#[test]
fn progress_is_strictly_increasing_and_omits_a_total() {
    // `progress` MUST increase per the spec, and `total` is omitted because an auth wait
    // is open-ended.
    let token = json!("tok");
    let first = progress(Some(&token), 1, &name());
    let second = progress(Some(&token), 2, &name());
    assert_eq!(first["params"]["progress"], 1);
    assert_eq!(second["params"]["progress"], 2);
    assert!(first["params"].get("total").is_none());
    assert_eq!(first["params"]["progressToken"], "tok");
    assert!(first.get("id").is_none(), "a notification carries no id");
}

#[test]
fn the_progress_message_names_the_server_the_user_must_act_on() {
    let token = json!(1);
    let value = progress(Some(&token), 1, &name());
    let message = value["params"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("linear"));
}

#[test]
fn the_login_message_names_the_exact_command() {
    let text = needs_login(&name());
    assert!(text.contains("opman mcp login linear"));
}

#[test]
fn the_timeout_message_still_tells_the_user_what_to_do() {
    let text = timed_out(&name());
    assert!(text.contains("opman mcp login linear"));
}
