//! Generated tests for `WebPtyHandle`.
//!
//! We construct a handle around our own mpsc channel, receive the emitted
//! `PtyCmd` on the other end, assert its fields, and drive the reply — so we
//! exercise every method without spawning a real PTY manager thread.

use super::*;
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::super::kind::{PtyKind, PtyProgram};

fn handle() -> (WebPtyHandle, mpsc::UnboundedReceiver<PtyCmd>) {
    let (cmd_tx, rx) = mpsc::unbounded_channel();
    (WebPtyHandle { cmd_tx }, rx)
}

fn a_spec(id: &str, program: PtyProgram) -> SpawnSpec {
    SpawnSpec {
        id: id.into(),
        program,
        project: PathBuf::from("/w"),
        label: None,
        rows: 24,
        cols: 80,
    }
}

#[tokio::test]
async fn spawn_sends_the_spec_and_returns_the_buffer() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.spawn(a_spec("id", PtyProgram::Shell)).await });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Spawn { spec, reply } => {
            assert_eq!(spec.id, "id");
            assert_eq!((spec.rows, spec.cols), (24, 80));
            assert_eq!(spec.project, PathBuf::from("/w"));
            assert_eq!(spec.program.kind(), PtyKind::Shell);
            let sent = reply.send(Ok(RawOutputBuffer::new()));
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    assert!(task.await.expect("the task joins").is_ok());
}

/// The per-kind arguments have to survive the trip, since nothing downstream
/// can reconstruct which session or agent was meant.
#[tokio::test]
async fn spawn_carries_per_kind_arguments() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move {
        h.spawn(a_spec(
            "o",
            PtyProgram::Opencode {
                session_id: Some("sid".into()),
            },
        ))
        .await
    });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Spawn { spec, reply } => {
            match &spec.program {
                PtyProgram::Opencode { session_id } => {
                    assert_eq!(session_id.as_deref(), Some("sid"))
                }
                _ => panic!("wrong program"),
            }
            let sent = reply.send(Ok(RawOutputBuffer::new()));
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    assert!(task.await.expect("the task joins").is_ok());
}

/// An error from the manager reaches the caller as its own message rather than
/// as a generic failure.
#[tokio::test]
async fn spawn_propagates_the_managers_error() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move {
        h.spawn(a_spec(
            "c",
            PtyProgram::ClaudeAttach {
                short_id: "short".into(),
            },
        ))
        .await
    });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Spawn { spec, reply } => {
            match &spec.program {
                PtyProgram::ClaudeAttach { short_id } => assert_eq!(short_id, "short"),
                _ => panic!("wrong program"),
            }
            let sent = reply.send(Err("boom".into()));
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    let outcome = task.await.expect("the task joins");
    assert_eq!(outcome.err().as_deref(), Some("boom"));
}

#[tokio::test]
async fn spawn_returns_error_when_manager_gone() {
    let (h, rx) = handle();
    drop(rx); // channel closed -> send fails
    let err = h.spawn(a_spec("id", PtyProgram::Shell)).await;
    assert_eq!(err.err().as_deref(), Some("PTY manager not running"));
}

#[tokio::test]
async fn spawn_returns_error_when_reply_dropped() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.spawn(a_spec("id", PtyProgram::Shell)).await });
    // Receive and drop the reply sender without answering.
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Spawn { reply, .. } => drop(reply),
        _ => panic!("wrong command"),
    }
    assert_eq!(
        task.await.expect("the task joins").err().as_deref(),
        Some("PTY manager dropped")
    );
}

#[tokio::test]
async fn write_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.write("id", vec![9, 9]).await });
    match rx.recv().await.unwrap() {
        PtyCmd::Write { id, data, reply } => {
            assert_eq!(id, "id");
            assert_eq!(data, vec![9, 9]);
            reply.send(true).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap());

    // send failure -> false
    let (h2, rx2) = handle();
    drop(rx2);
    assert!(!h2.write("id", vec![]).await);
}

#[tokio::test]
async fn write_reply_dropped_is_false() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.write("id", vec![1]).await });
    match rx.recv().await.unwrap() {
        PtyCmd::Write { reply, .. } => drop(reply),
        _ => panic!(),
    }
    assert!(!task.await.unwrap());
}

#[tokio::test]
async fn resize_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.resize("id", 30, 90).await });
    match rx.recv().await.unwrap() {
        PtyCmd::Resize {
            rows, cols, reply, ..
        } => {
            assert_eq!((rows, cols), (30, 90));
            reply.send(true).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap());

    let (h2, rx2) = handle();
    drop(rx2);
    assert!(!h2.resize("id", 1, 1).await);
}

#[tokio::test]
async fn get_output_present_and_absent() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.get_output("id").await });
    match rx.recv().await.unwrap() {
        PtyCmd::GetOutput { id, reply } => {
            assert_eq!(id, "id");
            reply.send(Some(RawOutputBuffer::new())).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap().is_some());

    // send failure -> None
    let (h2, rx2) = handle();
    drop(rx2);
    assert!(h2.get_output("id").await.is_none());
}

#[tokio::test]
async fn get_output_reply_dropped_is_none() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.get_output("id").await });
    match rx.recv().await.unwrap() {
        PtyCmd::GetOutput { reply, .. } => drop(reply),
        _ => panic!(),
    }
    assert!(task.await.unwrap().is_none());
}

#[tokio::test]
async fn kill_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.kill("id").await });
    match rx.recv().await.unwrap() {
        PtyCmd::Kill { id, reply } => {
            assert_eq!(id, "id");
            reply.send(true).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap());

    let (h2, rx2) = handle();
    drop(rx2);
    assert!(!h2.kill("id").await);
}

#[tokio::test]
async fn rename_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.rename("id", "Build".into()).await });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Rename { id, label, reply } => {
            assert_eq!((id.as_str(), label.as_str()), ("id", "Build"));
            let sent = reply.send(true);
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    assert!(task.await.expect("the task joins"));

    let (gone, rx2) = handle();
    drop(rx2);
    assert!(!gone.rename("id", "Build".into()).await);
}

#[tokio::test]
async fn sessions_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.sessions().await });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Sessions { reply } => {
            let sent = reply.send(vec![a_session("a"), a_session("b")]);
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    let listed = task.await.expect("the task joins");
    assert_eq!(listed.len(), 2);

    let (gone, rx2) = handle();
    drop(rx2);
    assert!(gone.sessions().await.is_empty());
}

/// `list` is a projection of `sessions`, so it must ask the same question and
/// keep only the ids.
#[tokio::test]
async fn list_returns_the_session_ids() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.list().await });
    match rx.recv().await.expect("a command is sent") {
        PtyCmd::Sessions { reply } => {
            let sent = reply.send(vec![a_session("a"), a_session("b")]);
            assert!(sent.is_ok());
        }
        _ => panic!("wrong command"),
    }
    assert_eq!(task.await.expect("the task joins"), ["a", "b"]);

    let (gone, rx2) = handle();
    drop(rx2);
    assert!(gone.list().await.is_empty());
}

fn a_session(id: &str) -> PtySession {
    PtySession {
        id: id.into(),
        kind: PtyKind::Shell,
        label: "Shell 1".into(),
        project: "/w".into(),
        activity: super::super::activity::PtyActivity::Idle,
    }
}
