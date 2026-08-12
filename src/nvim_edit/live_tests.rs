use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::nvim_edit::EditEngine;
use crate::nvim_ui::live::helpers::{fixture, have_nvim, lock, start};
use crate::nvim_ui::stream::wire::{ClientMsg, ControlMsg, ExCommand, ModeShort, TextPosition};

async fn setup(
    id: &str,
    contents: &str,
) -> (
    tempfile::TempDir,
    Arc<crate::nvim_ui::NvimSession>,
    Arc<EditEngine>,
    mpsc::UnboundedReceiver<ControlMsg>,
    tokio::task::JoinHandle<()>,
) {
    let project = fixture();
    std::fs::write(project.path().join("edit.txt"), contents).expect("fixture file");
    let (session, _) = start(&project, id).await;
    let (sender, receiver) = mpsc::unbounded_channel();
    let engine = EditEngine::new(session.clone(), sender);
    let task = tokio::spawn(engine.clone().notifications(session.subscribe()));
    engine
        .handle(ClientMsg::Attach {
            path: "edit.txt".into(),
        })
        .await
        .expect("attach edit buffer");
    (project, session, engine, receiver, task)
}

async fn next(receiver: &mut mpsc::UnboundedReceiver<ControlMsg>) -> ControlMsg {
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("edit event timeout")
        .expect("edit event channel closed")
}

async fn attached(receiver: &mut mpsc::UnboundedReceiver<ControlMsg>) -> (u64, Vec<String>) {
    loop {
        if let ControlMsg::Attached {
            changedtick, lines, ..
        } = next(receiver).await
        {
            return (changedtick, lines);
        }
    }
}

#[path = "live_command_tests.rs"]
mod command_tests;
#[path = "live_state_tests.rs"]
mod state_tests;

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn input_emits_exact_incremental_lines_event() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-input", "").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input {
            keys: "ihello<Esc>".into(),
        })
        .await
        .expect("input");
    loop {
        if let ControlMsg::BufferChanged {
            first_line,
            last_line,
            new_last_line,
            lines,
            ..
        } = next(&mut events).await
        {
            assert_eq!((first_line, last_line, new_last_line), (0, 1, 1));
            assert_eq!(lines, ["hello"]);
            break;
        }
    }
    let value = session
        .client()
        .request(
            "nvim_buf_get_lines",
            rmpv::Value::Array(vec![0.into(), 0.into(), (-1).into(), false.into()]),
        )
        .await
        .expect("buffer read");
    assert!(format!("{value:?}").contains("hello"));
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn client_edit_echo_is_marked_and_stale_edit_is_rejected() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-client", "foo").await;
    let (tick, _) = attached(&mut events).await;
    engine
        .handle(ClientMsg::Edit {
            changedtick: tick,
            start: TextPosition { line: 0, column: 0 },
            end: TextPosition { line: 0, column: 3 },
            lines: vec!["bar".into()],
            edit_id: "client-1".into(),
        })
        .await
        .expect("client edit");
    loop {
        if let ControlMsg::BufferChanged { origin, lines, .. } = next(&mut events).await {
            assert_eq!(origin.as_deref(), Some("client-1"));
            assert_eq!(lines, ["bar"]);
            break;
        }
    }
    engine
        .handle(ClientMsg::Edit {
            changedtick: tick.saturating_sub(1),
            start: TextPosition { line: 0, column: 0 },
            end: TextPosition { line: 0, column: 3 },
            lines: vec!["bad".into()],
            edit_id: "stale".into(),
        })
        .await
        .expect("stale edit response");
    loop {
        if let ControlMsg::ResyncRequired { .. } = next(&mut events).await {
            break;
        }
    }
    let value = session
        .client()
        .request(
            "nvim_buf_get_lines",
            rmpv::Value::Array(vec![0.into(), 0.into(), (-1).into(), false.into()]),
        )
        .await
        .expect("buffer read");
    assert!(!format!("{value:?}").contains("bad"));
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn structured_substitute_and_multibyte_cursor_round_trip() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-command", "foo foo\n").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Command {
            command: ExCommand::Substitute {
                pattern: "foo".into(),
                replacement: "bar".into(),
                global: true,
                ignore_case: false,
            },
        })
        .await
        .expect("substitute");
    loop {
        if let ControlMsg::BufferChanged { lines, .. } = next(&mut events).await {
            assert_eq!(lines, ["bar bar"]);
            break;
        }
    }
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn multibyte_cursor_reports_utf16_column() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-columns", "😀界e\u{301}").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input { keys: "l".into() })
        .await
        .expect("cursor motion");
    loop {
        if let ControlMsg::State {
            cursor, mode_short, ..
        } = next(&mut events).await
        {
            if mode_short == ModeShort::Normal {
                assert_eq!(cursor.column, 2);
                break;
            }
        }
    }
    task.abort();
    session.shutdown().await;
}
