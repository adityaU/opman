use std::sync::Arc;

use super::{attached, next, setup};
use crate::nvim_edit::EditEngine;
use crate::nvim_ui::live::helpers::{have_nvim, lock};
use crate::nvim_ui::stream::wire::{ClientMsg, ControlMsg};

async fn typed_command(engine: &Arc<EditEngine>, command: &str) {
    engine
        .handle(ClientMsg::Input { keys: ":".into() })
        .await
        .expect("command prefix");
    for character in command.chars() {
        engine
            .handle(ClientMsg::Input {
                keys: character.to_string(),
            })
            .await
            .expect("command character");
    }
    engine
        .handle(ClientMsg::Input {
            keys: "<CR>".into(),
        })
        .await
        .expect("command terminator");
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn typed_write_persists_the_attached_file() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (project, session, engine, mut events, task) = setup("edit-write", "before").await;
    let _ = attached(&mut events).await;
    engine
        .handle(ClientMsg::Input {
            keys: "A after<Esc>".into(),
        })
        .await
        .expect("modify buffer");
    typed_command(&engine, "w").await;
    assert_eq!(
        std::fs::read_to_string(project.path().join("edit.txt")).expect("saved file"),
        "before after\n"
    );
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn typed_sort_settles_the_change_before_command_output() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-sort", "c\na\nb\n").await;
    let _ = attached(&mut events).await;
    typed_command(&engine, "sort").await;
    let mut sorted = false;
    loop {
        match next(&mut events).await {
            ControlMsg::BufferChanged { lines, .. } => {
                if lines == ["a", "b", "c"] {
                    sorted = true;
                }
            }
            ControlMsg::CommandOutput { .. } => {
                assert!(sorted, "sort output arrived before its buffer change");
                break;
            }
            _ => {}
        }
    }
    task.abort();
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn typed_buffer_delete_settles_as_a_detach() {
    if !have_nvim() {
        return;
    }
    let _guard = lock().await;
    let (_project, session, engine, mut events, task) = setup("edit-delete", "clean").await;
    let _ = attached(&mut events).await;
    let buffer = session
        .client()
        .request("nvim_get_current_buf", rmpv::Value::Array(Vec::new()))
        .await
        .expect("current buffer");
    let buffer = crate::nvim_ui::rpc::value::ext_or_int(&buffer).expect("buffer handle");
    typed_command(&engine, "bd").await;
    loop {
        if let ControlMsg::BufferDetached {
            buffer: detached, ..
        } = next(&mut events).await
        {
            assert_eq!(detached, buffer as u64);
            break;
        }
    }
    let loaded = session
        .client()
        .request(
            "nvim_buf_is_loaded",
            rmpv::Value::Array(vec![rmpv::Value::from(buffer)]),
        )
        .await
        .expect("buffer loaded state");
    assert_eq!(loaded.as_bool(), Some(false));
    task.abort();
    session.shutdown().await;
}
