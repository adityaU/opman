//! The bookkeeping that keeps background callers on the moved conversation.

use serde_json::json;

use super::*;

#[test]
fn a_forward_chain_is_followed_to_its_end() {
    let watch = LimitWatch::default();
    assert_eq!(watch.forward_of("a"), None);
    watch.record_forward("a", "b");
    watch.record_forward("b", "c");
    assert_eq!(watch.forward_of("a").as_deref(), Some("c"));
    assert_eq!(watch.forward_of("b").as_deref(), Some("c"));
    // A move onto itself is not a move.
    watch.record_forward("c", "c");
    assert_eq!(watch.forward_of("c"), None);
}

#[test]
fn a_failure_is_acted_on_once() {
    let watch = LimitWatch::default();
    assert!(watch.first_time("msg_1"));
    assert!(!watch.first_time("msg_1"));
    assert!(watch.first_time("msg_2"));
}

#[test]
fn retargeting_drops_the_old_runners_choices_and_names_the_new_model() {
    let mut body = json!({"parts":[], "agent":"build", "effort":"high",
        "model":{"providerID":"anthropic","modelID":"opus"}});
    let pair = FallbackPair {
        runner: "codex".into(),
        model: Some(super::super::settings::ModelRef {
            provider_id: "openai".into(),
            model_id: "gpt-5.6-sol".into(),
        }),
    };
    retarget(&mut body, &pair);
    assert_eq!(body, json!({"parts":[], "model":{"providerID":"openai","modelID":"gpt-5.6-sol"}}));

    retarget(&mut body, &FallbackPair { runner: "opencode".into(), model: None });
    assert_eq!(body, json!({"parts":[]}));
}
