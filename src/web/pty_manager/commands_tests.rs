//! Generated tests for PtyCmd enum construction / field access.
//!
//! `PtyCmd` has no executable methods; these tests simply construct each
//! variant and destructure it, ensuring the variant shapes stay stable.

use super::*;
use std::path::PathBuf;
use tokio::sync::oneshot;

#[test]
fn constructs_and_matches_spawn_variants() {
    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::SpawnShell {
        id: "id-1".into(),
        rows: 24,
        cols: 80,
        working_dir: PathBuf::from("/tmp"),
        reply: tx,
    };
    match cmd {
        PtyCmd::SpawnShell { id, rows, cols, working_dir, .. } => {
            assert_eq!(id, "id-1");
            assert_eq!(rows, 24);
            assert_eq!(cols, 80);
            assert_eq!(working_dir, PathBuf::from("/tmp"));
        }
        _ => panic!("wrong variant"),
    }

    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::SpawnOpencode {
        id: "o".into(),
        rows: 10,
        cols: 20,
        working_dir: PathBuf::from("/w"),
        session_id: Some("sess".into()),
        reply: tx,
    };
    if let PtyCmd::SpawnOpencode { session_id, .. } = cmd {
        assert_eq!(session_id.as_deref(), Some("sess"));
    } else {
        panic!("wrong variant");
    }

    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::SpawnClaudeAttach {
        id: "c".into(),
        rows: 1,
        cols: 2,
        working_dir: PathBuf::from("/"),
        short_id: "abc123".into(),
        reply: tx,
    };
    if let PtyCmd::SpawnClaudeAttach { short_id, .. } = cmd {
        assert_eq!(short_id, "abc123");
    } else {
        panic!("wrong variant");
    }

    // SpawnNeovim + SpawnGitui share the shape.
    let (tx, _rx) = oneshot::channel();
    let _ = PtyCmd::SpawnNeovim { id: "n".into(), rows: 5, cols: 5, working_dir: PathBuf::from("."), reply: tx };
    let (tx, _rx) = oneshot::channel();
    let _ = PtyCmd::SpawnGitui { id: "g".into(), rows: 5, cols: 5, working_dir: PathBuf::from("."), reply: tx };
}

#[test]
fn constructs_and_matches_control_variants() {
    let (tx, _rx) = oneshot::channel::<bool>();
    if let PtyCmd::Write { id, data, .. } = (PtyCmd::Write { id: "w".into(), data: vec![1, 2, 3], reply: tx }) {
        assert_eq!(id, "w");
        assert_eq!(data, vec![1, 2, 3]);
    } else {
        panic!();
    }

    let (tx, _rx) = oneshot::channel::<bool>();
    if let PtyCmd::Resize { rows, cols, .. } = (PtyCmd::Resize { id: "r".into(), rows: 40, cols: 100, reply: tx }) {
        assert_eq!((rows, cols), (40, 100));
    } else {
        panic!();
    }

    let (tx, _rx) = oneshot::channel::<Option<RawOutputBuffer>>();
    let _ = PtyCmd::GetOutput { id: "o".into(), reply: tx };

    let (tx, _rx) = oneshot::channel::<bool>();
    let _ = PtyCmd::Kill { id: "k".into(), reply: tx };

    let (tx, _rx) = oneshot::channel::<Vec<String>>();
    let _ = PtyCmd::List { reply: tx };
}

#[test]
fn get_output_reply_carries_buffer() {
    // Confirms RawOutputBuffer flows through the GetOutput reply channel type.
    let (tx, mut rx) = oneshot::channel::<Option<RawOutputBuffer>>();
    let cmd = PtyCmd::GetOutput { id: "x".into(), reply: tx };
    if let PtyCmd::GetOutput { reply, .. } = cmd {
        reply.send(Some(RawOutputBuffer::new())).unwrap();
    }
    assert!(rx.try_recv().unwrap().is_some());
}
