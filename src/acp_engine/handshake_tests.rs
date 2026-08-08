use super::*;

/// The common case, and the one that must not start refusing connections: agents answer with
/// the version opman asked for.
#[test]
fn the_agreed_version_is_the_one_opman_speaks() {
    assert!(agreed_version(&json!({ "protocolVersion": 1 })).is_ok());
}

/// An agent from before the field existed can only be v1, so silence is not a failure.
#[test]
fn a_missing_version_is_treated_as_v1() {
    assert!(agreed_version(&json!({})).is_ok());
}

/// ACP has the agent answer with the newest revision it speaks that is no newer than the
/// client's. Anything higher means it will send frames opman does not understand, and saying
/// so once at the handshake beats discovering it one missing field at a time.
#[test]
fn a_newer_agent_is_refused_at_the_handshake() {
    let refused = agreed_version(&json!({ "protocolVersion": 2 }));
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("v2"), "{message}");
    assert!(message.contains("v1"), "{message}");
}

/// An *older* agent is not refused: every capability opman reads defaults to unsupported
/// when absent, so a lower version degrades rather than breaks.
#[test]
fn an_older_agent_is_accepted() {
    assert!(agreed_version(&json!({ "protocolVersion": 0 })).is_ok());
}

#[test]
fn load_capability_is_read_from_agent_capabilities() {
    assert!(supports_load(
        &json!({ "agentCapabilities": { "loadSession": true } })
    ));
    assert!(!supports_load(&json!({ "agentCapabilities": {} })));
    assert!(!supports_load(&json!({})));
}

/// Both spellings seen in the wild have to count, or a steerable agent gets its follow-ups
/// queued behind the turn they were meant to interrupt.
#[test]
fn steering_is_recognised_in_either_spelling() {
    assert!(advertises_steering(
        &json!({ "_meta": { "steering": { "supported": true } } })
    ));
    assert!(advertises_steering(&json!({
        "agentCapabilities": { "_meta": { "claudeCode": { "promptQueueing": true } } }
    })));
    assert!(!advertises_steering(&json!({ "agentCapabilities": {} })));
}

/// opman logs in with the first method the agent lists.
#[test]
fn the_first_advertised_auth_method_is_chosen() {
    let init = json!({
        "authMethods": [
            { "id": "oauth", "name": "Log in with the browser" },
            { "id": "api-key", "name": "Paste an API key" },
        ]
    });
    assert_eq!(first_auth_method(&init), Some("oauth"));
}

/// An id is what `authenticate` is keyed on, so an entry without one is not a choice.
#[test]
fn auth_methods_without_an_id_are_skipped() {
    let init = json!({ "authMethods": [{ "name": "nameless" }, { "id": "", }, { "id": "real" }] });
    assert_eq!(first_auth_method(&init), Some("real"));
}

/// An agent demanding authentication while advertising no way to do it cannot be helped from
/// here; the caller turns this into a message naming its own CLI.
#[test]
fn no_auth_methods_means_nothing_to_send() {
    assert_eq!(first_auth_method(&json!({ "authMethods": [] })), None);
    assert_eq!(first_auth_method(&json!({})), None);
}
