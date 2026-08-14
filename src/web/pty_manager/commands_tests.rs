//! Tests for PtyCmd construction / field access.
//!
//! `PtyCmd` has no executable methods; these tests construct each variant and
//! destructure it, ensuring the variant shapes stay stable.

use super::*;
use std::path::PathBuf;
use tokio::sync::oneshot;

use super::super::kind::{PtyKind, PtyProgram};

fn a_spec(program: PtyProgram) -> SpawnSpec {
    SpawnSpec {
        id: "id-1".into(),
        program,
        project: PathBuf::from("/tmp"),
        label: None,
        rows: 24,
        cols: 80,
    }
}

/// One variant now carries every kind, and the per-kind arguments ride on the
/// program rather than on the command.
#[test]
fn spawn_carries_the_whole_spec() {
    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::Spawn {
        spec: Box::new(a_spec(PtyProgram::Shell)),
        reply: tx,
    };
    let PtyCmd::Spawn { spec, .. } = cmd else {
        panic!("wrong variant");
    };
    assert_eq!(spec.id, "id-1");
    assert_eq!(spec.rows, 24);
    assert_eq!(spec.cols, 80);
    assert_eq!(spec.project, PathBuf::from("/tmp"));
    assert_eq!(spec.program.kind(), PtyKind::Shell);
    assert!(spec.label.is_none(), "the manager numbers it");
}

#[test]
fn spawn_carries_per_kind_arguments() {
    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::Spawn {
        spec: Box::new(a_spec(PtyProgram::Opencode {
            session_id: Some("sess".into()),
        })),
        reply: tx,
    };
    let PtyCmd::Spawn { spec, .. } = cmd else {
        panic!("wrong variant");
    };
    match &spec.program {
        PtyProgram::Opencode { session_id } => assert_eq!(session_id.as_deref(), Some("sess")),
        _ => panic!("wrong program"),
    }

    let (tx, _rx) = oneshot::channel();
    let cmd = PtyCmd::Spawn {
        spec: Box::new(a_spec(PtyProgram::ClaudeAttach {
            short_id: "abc123".into(),
        })),
        reply: tx,
    };
    let PtyCmd::Spawn { spec, .. } = cmd else {
        panic!("wrong variant");
    };
    match &spec.program {
        PtyProgram::ClaudeAttach { short_id } => assert_eq!(short_id, "abc123"),
        _ => panic!("wrong program"),
    }
}

#[test]
fn constructs_and_matches_control_variants() {
    let (tx, _rx) = oneshot::channel::<bool>();
    if let PtyCmd::Write { id, data, .. } = (PtyCmd::Write {
        id: "w".into(),
        data: vec![1, 2, 3],
        reply: tx,
    }) {
        assert_eq!(id, "w");
        assert_eq!(data, vec![1, 2, 3]);
    } else {
        panic!();
    }

    let (tx, _rx) = oneshot::channel::<bool>();
    if let PtyCmd::Resize { rows, cols, .. } = (PtyCmd::Resize {
        id: "r".into(),
        rows: 40,
        cols: 100,
        reply: tx,
    }) {
        assert_eq!((rows, cols), (40, 100));
    } else {
        panic!();
    }

    let (tx, _rx) = oneshot::channel::<bool>();
    if let PtyCmd::Rename { id, label, .. } = (PtyCmd::Rename {
        id: "n".into(),
        label: "Build".into(),
        reply: tx,
    }) {
        assert_eq!((id.as_str(), label.as_str()), ("n", "Build"));
    } else {
        panic!();
    }

    let (tx, _rx) = oneshot::channel::<Option<RawOutputBuffer>>();
    let _ = PtyCmd::GetOutput {
        id: "o".into(),
        reply: tx,
    };

    let (tx, _rx) = oneshot::channel::<bool>();
    let _ = PtyCmd::Kill {
        id: "k".into(),
        reply: tx,
    };

    let (tx, _rx) = oneshot::channel::<Vec<PtySession>>();
    let _ = PtyCmd::Sessions { reply: tx };
}

#[test]
fn get_output_reply_carries_buffer() {
    // Confirms RawOutputBuffer flows through the GetOutput reply channel type.
    let (tx, mut rx) = oneshot::channel::<Option<RawOutputBuffer>>();
    let cmd = PtyCmd::GetOutput {
        id: "x".into(),
        reply: tx,
    };
    if let PtyCmd::GetOutput { reply, .. } = cmd {
        let sent = reply.send(Some(RawOutputBuffer::new()));
        assert!(sent.is_ok(), "the receiver is still alive");
    }
    assert!(rx.try_recv().is_ok_and(|buffer| buffer.is_some()));
}
