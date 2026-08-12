use super::{attached, have_nvim, lock, next, setup};
use crate::nvim_ui::stream::wire::{ClientMsg, ControlMsg, ModeShort, NvimMode, TextPosition};

async fn state_with_mode(
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<ControlMsg>,
    expected: ModeShort,
) -> (
    NvimMode,
    ModeShort,
    TextPosition,
    Option<crate::nvim_ui::stream::wire::VisualSelection>,
) {
    loop {
        if let ControlMsg::State {
            mode,
            mode_short,
            cursor,
            visual,
            ..
        } = next(receiver).await
        {
            if mode_short == expected {
                return (mode, mode_short, cursor, visual);
            }
        }
    }
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn change_operator_reports_insert_mode_after_input_settles() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) =
        setup("edit-change-mode", "hello world").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input { keys: "cw".into() })
        .await
        .expect("change operator input");
    let (mode, short, _, visual) = state_with_mode(&mut events, ModeShort::Insert).await;
    assert_eq!(mode, NvimMode::Insert);
    assert_eq!(short, ModeShort::Insert);
    assert!(visual.is_none());
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn visual_modes_report_ranges_for_character_line_and_block() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) =
        setup("edit-visual-modes", "abc\ndef\nghi").await;
    let _ = attached(&mut events).await;

    engine
        .handle(ClientMsg::Input { keys: "vl".into() })
        .await
        .expect("visual character input");
    let (mode, _, _, visual) = state_with_mode(&mut events, ModeShort::Visual).await;
    assert_eq!(mode, NvimMode::Visual);
    let selection = visual.expect("character selection");
    assert_eq!(selection.start, TextPosition { line: 0, column: 0 });
    assert_eq!(selection.end, TextPosition { line: 0, column: 1 });

    engine
        .handle(ClientMsg::Input {
            keys: "<Esc>Vj".into(),
        })
        .await
        .expect("visual line input");
    let (mode, _, _, visual) = state_with_mode(&mut events, ModeShort::VisualLine).await;
    assert_eq!(mode, NvimMode::VisualLine);
    let selection = visual.expect("line selection");
    assert_eq!(selection.start, TextPosition { line: 0, column: 0 });
    assert_eq!(selection.end, TextPosition { line: 1, column: 0 });

    engine
        .handle(ClientMsg::Input {
            keys: "<Esc><C-v>l".into(),
        })
        .await
        .expect("visual block input");
    let (mode, _, _, visual) = state_with_mode(&mut events, ModeShort::VisualBlock).await;
    assert_eq!(mode, NvimMode::VisualBlock);
    let selection = visual.expect("block selection");
    // Neovim keeps the cursor at column 1 after the preceding characterwise
    // selection, so the following blockwise selection starts there.
    assert_eq!(selection.start, TextPosition { line: 1, column: 1 });
    assert_eq!(selection.end, TextPosition { line: 1, column: 2 });

    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn operator_pending_reports_before_motion() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) =
        setup("edit-operator-pending", "hello").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input { keys: "d".into() })
        .await
        .expect("operator input");
    let (mode, short, _, visual) = state_with_mode(&mut events, ModeShort::OperatorPending).await;
    assert_eq!(mode, NvimMode::OperatorPending);
    assert_eq!(short, ModeShort::OperatorPending);
    assert!(visual.is_none());
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn replace_reports_replace_mode() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-replace-mode", "hello").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input { keys: "R".into() })
        .await
        .expect("replace input");
    let (mode, short, _, visual) = state_with_mode(&mut events, ModeShort::Replace).await;
    assert_eq!(mode, NvimMode::Replace);
    assert_eq!(short, ModeShort::Replace);
    assert!(visual.is_none());
    task.abort();
    session.shutdown().await;
}
