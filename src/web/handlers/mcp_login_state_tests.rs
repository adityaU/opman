//! Where a pasted OAuth callback may be delivered.
//!
//! No authorization server is involved in any of this, which is the point: the decision
//! about the delivery address is made from the flow's own authorize URL, so it is testable
//! — and it is the decision that would be a request-forgery hole if it were wrong.

use super::*;

use crate::claude_engine::claude_cli::ENV_LOCK;

// ── the redirect a paste may be delivered to ────────────────────────────────────────

#[test]
fn redirect_comes_from_the_authorize_url() {
    let redirect = Redirect::from_authorize(
        "https://auth.example.com/authorize?client_id=x&redirect_uri=http%3A%2F%2F127.0.0.1%3A54321%2Fcallback",
    )
    .expect("loopback redirect");
    assert_eq!(redirect.as_str(), "http://127.0.0.1:54321/callback");
}

#[test]
fn redirect_rejects_anything_off_loopback() {
    // A public redirect would put an authorization code on a tunnel hostname, and
    // delivering a paste to one would make the finish endpoint fetch an arbitrary URL.
    for encoded in [
        "https%3A%2F%2Fevil.example.com%2Fcallback",
        "http%3A%2F%2Fevil.example.com%3A80%2Fcallback",
        "http%3A%2F%2F10.0.0.5%3A8080%2Fcallback",
        // Loopback, but with no port — nothing listens on a port we did not bind.
        "http%3A%2F%2F127.0.0.1%2Fcallback",
    ] {
        let authorize = format!("https://auth.example.com/authorize?redirect_uri={encoded}");
        assert!(
            Redirect::from_authorize(&authorize).is_none(),
            "should reject {encoded}"
        );
    }
}

#[test]
fn redirect_rejects_an_authorize_url_without_one() {
    assert!(Redirect::from_authorize("https://auth.example.com/authorize?state=x").is_none());
    assert!(Redirect::from_authorize("not a url").is_none());
}

#[test]
fn delivery_keeps_the_listener_address_and_takes_only_the_query() {
    let redirect = Redirect::from_authorize(
        "https://a.example/authorize?redirect_uri=http%3A%2F%2F127.0.0.1%3A9999%2Fcallback",
    )
    .expect("loopback redirect");
    let delivery = redirect.delivery("code=abc&state=xyz");
    assert_eq!(delivery.host_str(), Some("127.0.0.1"));
    assert_eq!(delivery.port(), Some(9999));
    assert_eq!(delivery.path(), "/callback");
    assert_eq!(delivery.query(), Some("code=abc&state=xyz"));
}

// ── what counts as a pasted callback ────────────────────────────────────────────────

#[test]
fn callback_query_accepts_the_forms_a_user_actually_pastes() {
    let full = callback_query("http://127.0.0.1:1234/callback?code=abc&state=xyz");
    assert_eq!(full.as_deref(), Some("code=abc&state=xyz"));

    let bare = callback_query("code=abc&state=xyz");
    assert_eq!(bare.as_deref(), Some("code=abc&state=xyz"));

    let padded = callback_query("  http://localhost:1/callback?code=a&state=b  ");
    assert_eq!(padded.as_deref(), Some("code=a&state=b"));

    let fragment = callback_query("http://127.0.0.1:1/callback?code=a&state=b#done");
    assert_eq!(fragment.as_deref(), Some("code=a&state=b"));
}

#[test]
fn callback_query_rejects_input_with_no_parameters() {
    assert!(callback_query("").is_none());
    assert!(callback_query("http://127.0.0.1:1/callback").is_none());
    assert!(callback_query("   ").is_none());
}

// ── secrets named rather than stored ────────────────────────────────────────────────

#[test]
fn env_references_resolve_and_literals_pass_through() {
    let _lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::env::set_var("OPMAN_TEST_MCP_SECRET", "s3cret");
    assert_eq!(resolve_env("${env:OPMAN_TEST_MCP_SECRET}"), "s3cret");
    assert_eq!(resolve_env("literal-secret"), "literal-secret");
    assert_eq!(resolve_env("${env:OPMAN_TEST_MCP_ABSENT}"), "");
    std::env::remove_var("OPMAN_TEST_MCP_SECRET");
}

// ── the pending-flow table ──────────────────────────────────────────────────────────

fn loopback() -> Redirect {
    Redirect::from_authorize(
        "https://a.example/authorize?redirect_uri=http%3A%2F%2F127.0.0.1%3A7%2Fcallback",
    )
    .expect("loopback redirect")
}

#[tokio::test]
async fn arming_a_second_login_replaces_and_cancels_the_first() {
    let sessions = LoginSessions::default();
    // The sender is dropped only when the task itself ends, so the receiver erroring is
    // proof of cancellation rather than a guess about scheduling.
    let (alive, ended) = tokio::sync::oneshot::channel::<()>();
    let first = tokio::spawn(async move {
        let _alive = alive;
        // Long enough that only cancellation can end it.
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        Ok(())
    });
    sessions
        .arm(
            "linear",
            Pending {
                redirect: loopback(),
                task: first,
            },
        )
        .await;
    sessions
        .arm(
            "linear",
            Pending {
                redirect: loopback(),
                task: tokio::spawn(async { Ok(()) }),
            },
        )
        .await;

    // The first flow's loopback port must not stay bound for the browser timeout.
    let cancelled = tokio::time::timeout(std::time::Duration::from_secs(5), ended)
        .await
        .expect("first flow should be cancelled promptly");
    assert!(cancelled.is_err(), "first flow was left running");

    assert!(sessions.take("linear").await.is_some());
    assert!(sessions.take("linear").await.is_none());
}

#[tokio::test]
async fn disarming_cancels_without_leaving_an_entry_behind() {
    let sessions = LoginSessions::default();
    let (alive, ended) = tokio::sync::oneshot::channel::<()>();
    sessions
        .arm(
            "linear",
            Pending {
                redirect: loopback(),
                task: tokio::spawn(async move {
                    let _alive = alive;
                    tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                    Ok(())
                }),
            },
        )
        .await;

    sessions.disarm("linear").await;
    let cancelled = tokio::time::timeout(std::time::Duration::from_secs(5), ended)
        .await
        .expect("flow should be cancelled promptly");
    assert!(cancelled.is_err(), "flow was left running");
    assert!(sessions.take("linear").await.is_none());
}
