use super::*;

fn answers(picked: &[&str]) -> Answers {
    vec![picked.iter().map(|s| s.to_string()).collect()]
}

#[tokio::test]
async fn resolve_delivers_the_answer_to_the_waiter() {
    let pending = AskPending::default();
    let rx = pending.register("q1", "ses_a");

    assert!(pending.resolve("q1", answers(&["Postgres"])).is_ok());
    assert_eq!(rx.await.expect("answered"), answers(&["Postgres"]));
    // Answering retires the request, so a second reply has nothing to resolve and the
    // caller keeps fanning out instead of reporting a false success. The answers come
    // back rather than being dropped, so the fan-out still has something to send.
    assert_eq!(
        pending.resolve("q1", answers(&["SQLite"])),
        Err(answers(&["SQLite"]))
    );
}

#[tokio::test]
async fn dismiss_closes_the_channel_without_an_answer() {
    let pending = AskPending::default();
    let rx = pending.register("q2", "ses_a");

    assert!(pending.dismiss("q2"));
    assert!(rx.await.is_err(), "a dismissed question yields no answer");
    assert!(!pending.dismiss("q2"));
}

#[tokio::test]
async fn clear_session_only_touches_that_session() {
    let pending = AskPending::default();
    let mine = pending.register("q3", "ses_a");
    let theirs = pending.register("q4", "ses_b");

    assert_eq!(pending.clear_session("ses_a"), vec!["q3".to_string()]);
    assert!(mine.await.is_err());

    assert!(pending.resolve("q4", answers(&["yes"])).is_ok());
    assert_eq!(theirs.await.expect("answered"), answers(&["yes"]));
}

#[test]
fn clearing_an_idle_session_is_a_no_op() {
    let pending = AskPending::default();
    let _rx = pending.register("q5", "ses_a");
    assert!(pending.clear_session("ses_nothing").is_empty());
    // q5 is untouched, so dismissing it still finds something to dismiss.
    assert!(pending.dismiss("q5"));
}

#[tokio::test]
async fn a_dropped_receiver_makes_resolve_report_failure() {
    let pending = AskPending::default();
    drop(pending.register("q6", "ses_a"));
    // The request is still registered, but nobody is listening: the reply route must not
    // claim it answered the asker, and gets the answers back to fan out.
    assert_eq!(
        pending.resolve("q6", answers(&["yes"])),
        Err(answers(&["yes"]))
    );
}
