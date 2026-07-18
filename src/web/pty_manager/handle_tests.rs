//! Generated tests for `WebPtyHandle`.
//!
//! We construct a handle around our own mpsc channel, receive the emitted
//! `PtyCmd` on the other end, assert its fields, and drive the reply — so we
//! exercise every method without spawning a real PTY manager thread.

use super::*;
use std::path::PathBuf;
use tokio::sync::mpsc;

fn handle() -> (WebPtyHandle, mpsc::UnboundedReceiver<PtyCmd>) {
    let (cmd_tx, rx) = mpsc::unbounded_channel();
    (WebPtyHandle { cmd_tx }, rx)
}

#[tokio::test]
async fn spawn_shell_sends_command_and_returns_buffer() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move {
        h.spawn_shell("id".into(), 24, 80, PathBuf::from("/w")).await
    });
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnShell { id, rows, cols, working_dir, reply } => {
            assert_eq!(id, "id");
            assert_eq!((rows, cols), (24, 80));
            assert_eq!(working_dir, PathBuf::from("/w"));
            reply.send(Ok(RawOutputBuffer::new())).unwrap();
        }
        _ => panic!("wrong command"),
    }
    assert!(task.await.unwrap().is_ok());
}

#[tokio::test]
async fn spawn_neovim_sends_command() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.spawn_neovim("n".into(), 1, 2, PathBuf::from(".")).await });
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnNeovim { id, reply, .. } => {
            assert_eq!(id, "n");
            reply.send(Ok(RawOutputBuffer::new())).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap().is_ok());
}

#[tokio::test]
async fn spawn_gitui_sends_command() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.spawn_gitui("g".into(), 1, 2, PathBuf::from(".")).await });
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnGitui { reply, .. } => reply.send(Ok(RawOutputBuffer::new())).unwrap(),
        _ => panic!(),
    }
    assert!(task.await.unwrap().is_ok());
}

#[tokio::test]
async fn spawn_opencode_passes_session_id() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move {
        h.spawn_opencode("o".into(), 3, 4, PathBuf::from("/d"), Some("sid".into())).await
    });
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnOpencode { session_id, reply, .. } => {
            assert_eq!(session_id.as_deref(), Some("sid"));
            reply.send(Ok(RawOutputBuffer::new())).unwrap();
        }
        _ => panic!(),
    }
    assert!(task.await.unwrap().is_ok());
}

#[tokio::test]
async fn spawn_claude_attach_passes_short_id() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move {
        h.spawn_claude_attach("c".into(), 3, 4, PathBuf::from("/d"), "short".into()).await
    });
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnClaudeAttach { short_id, reply, .. } => {
            assert_eq!(short_id, "short");
            reply.send(Err("boom".into())).unwrap();
        }
        _ => panic!(),
    }
    // Manager replied with an error string — propagated through.
    assert_eq!(task.await.unwrap().unwrap_err(), "boom");
}

#[tokio::test]
async fn spawn_returns_error_when_manager_gone() {
    let (h, rx) = handle();
    drop(rx); // channel closed -> send fails
    let err = h.spawn_shell("id".into(), 1, 1, PathBuf::from(".")).await.unwrap_err();
    assert_eq!(err, "PTY manager not running");
}

#[tokio::test]
async fn spawn_returns_error_when_reply_dropped() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.spawn_shell("id".into(), 1, 1, PathBuf::from(".")).await });
    // Receive and drop the reply sender without answering.
    match rx.recv().await.unwrap() {
        PtyCmd::SpawnShell { reply, .. } => drop(reply),
        _ => panic!(),
    }
    assert_eq!(task.await.unwrap().unwrap_err(), "PTY manager dropped");
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
        PtyCmd::Resize { rows, cols, reply, .. } => {
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
async fn list_success_and_failure() {
    let (h, mut rx) = handle();
    let task = tokio::spawn(async move { h.list().await });
    match rx.recv().await.unwrap() {
        PtyCmd::List { reply } => reply.send(vec!["a".into(), "b".into()]).unwrap(),
        _ => panic!(),
    }
    assert_eq!(task.await.unwrap(), vec!["a".to_string(), "b".to_string()]);

    let (h2, rx2) = handle();
    drop(rx2);
    assert!(h2.list().await.is_empty());
}
