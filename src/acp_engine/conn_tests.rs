//! Session setup against an in-process agent, for the parts of it that are a round-trip.

use super::*;

use std::sync::Mutex as StdMutex;

/// Records the methods the agent was asked for, in order.
fn recorder() -> Arc<StdMutex<Vec<String>>> {
    Arc::new(StdMutex::new(Vec::new()))
}

fn calls(log: &Arc<StdMutex<Vec<String>>>) -> Vec<String> {
    log.lock().map(|seen| seen.clone()).unwrap_or_default()
}

/// ACP puts authentication behind a rejection rather than a capability flag: the agent refuses
/// `session/new` with `auth_required`, and expects the client to log in and try again. opman
/// used to surface that refusal as a raw error with nothing the user could do about it.
#[tokio::test]
async fn a_refused_session_is_retried_after_authenticating() {
    let log = recorder();
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        let mut seen = seen.lock().map_err(|_| json!({ "code": -1 }))?;
        seen.push(method.to_string());
        let attempts = seen.iter().filter(|m| *m == "session/new").count();
        match method {
            "session/new" if attempts == 1 => Err(json!({
                "code": jsonrpc::AUTH_REQUIRED, "message": "Authentication required",
            })),
            "session/new" => Ok(json!({ "sessionId": "s-1" })),
            _ => Ok(json!({})),
        }
    });

    let init = json!({ "authMethods": [{ "id": "oauth", "name": "Log in with the browser" }] });
    let (id, _setup) = open_session(&peer, &init, "/tmp", &json!([]))
        .await
        .expect("the session should open after logging in");

    assert_eq!(id, "s-1");
    assert_eq!(
        calls(&log),
        vec!["session/new", "authenticate", "session/new"]
    );
}

/// The login uses the id the agent published, since that is all `authenticate` is keyed on.
#[tokio::test]
async fn the_advertised_method_id_is_what_gets_sent() {
    let log = recorder();
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, params| {
        if method == "authenticate" {
            let id = params.get("methodId").and_then(Value::as_str).unwrap_or("");
            if let Ok(mut seen) = seen.lock() {
                seen.push(id.to_string());
            }
            return Ok(json!({}));
        }
        let first = seen.lock().map(|s| s.is_empty()).unwrap_or(false);
        match first {
            true => Err(json!({ "code": jsonrpc::AUTH_REQUIRED, "message": "log in" })),
            false => Ok(json!({ "sessionId": "s-1" })),
        }
    });

    let init = json!({ "authMethods": [{ "id": "api-key" }] });
    open_session(&peer, &init, "/tmp", &json!([]))
        .await
        .expect("session");
    assert_eq!(calls(&log), vec!["api-key"]);
}

/// An agent demanding a login while advertising no way to perform one cannot be helped from
/// here, so the message has to point at the thing that can: its own CLI.
#[tokio::test]
async fn a_login_with_no_advertised_method_says_where_to_go() {
    let peer = jsonrpc::fake_agent(|_method, _params| {
        Err(json!({ "code": jsonrpc::AUTH_REQUIRED, "message": "Authentication required" }))
    });

    let refused = open_session(&peer, &json!({}), "/tmp", &json!([])).await;
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("authMethods"), "{message}");
    assert!(message.contains("own CLI"), "{message}");
}

/// Only `auth_required` means "try again". Any other refusal is a real failure, and retrying
/// it would double every error the agent reports.
#[tokio::test]
async fn an_ordinary_failure_is_not_retried() {
    let log = recorder();
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        Err(json!({ "code": -32603, "message": "no such directory" }))
    });

    let refused = open_session(&peer, &json!({}), "/tmp", &json!([])).await;
    assert!(refused.is_err());
    assert_eq!(calls(&log), vec!["session/new"]);
}

/// The ordinary path stays one request.
#[tokio::test]
async fn an_agent_that_needs_no_login_is_asked_once() {
    let log = recorder();
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        Ok(json!({ "sessionId": "s-1", "configOptions": [] }))
    });

    let (id, setup) = open_session(&peer, &json!({}), "/tmp", &json!([]))
        .await
        .expect("session");
    assert_eq!(id, "s-1");
    assert!(setup.get("configOptions").is_some());
    assert_eq!(calls(&log), vec!["session/new"]);
}

/// A reply with no session id is not a session, however successful the call looked.
#[tokio::test]
async fn a_reply_without_a_session_id_is_an_error() {
    let peer = jsonrpc::fake_agent(|_method, _params| Ok(json!({})));
    assert!(open_session(&peer, &json!({}), "/tmp", &json!([]))
        .await
        .is_err());
}
