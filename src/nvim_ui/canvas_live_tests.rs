//! Backend UI-channel coverage retained for the edit engine's notifications.
use rmpv::Value;
use std::time::Duration;

use super::super::key::UiSize;
use super::helpers::{
    event, fixture, grid_contains, grid_contains_hello, grid_has_row, grid_resize_100x30,
    has_event, have_nvim, input_notify, lock, start, RedrawStream,
};

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn attach_emits_the_real_initial_ui_batch() {
    if !have_nvim() {
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (session, _) = start(&project, "attach").await;
    let mut redraws = RedrawStream::new(session.subscribe());
    session.reattach().await.expect("reattach");
    let required = [
        "grid_resize",
        "default_colors_set",
        "hl_attr_define",
        "mode_info_set",
        "flush",
    ];
    let mut seen = [false; 5];
    let mut last_events = Vec::new();
    for _ in 0..20 {
        let (events, _) = redraws.next("initial UI redraw events").await;
        for (index, name) in required.iter().enumerate() {
            seen[index] |= event(&events, name).is_some();
        }
        last_events = events;
        if seen.iter().all(|value| *value) {
            break;
        }
    }
    for (index, name) in required.iter().enumerate() {
        assert!(seen[index], "initial redraw lacks {name}: {last_events:?}");
    }
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn input_changes_both_the_real_grid_and_buffer() {
    if !have_nvim() {
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (session, _) = start(&project, "input").await;
    let mut redraws = RedrawStream::new(session.subscribe());
    session.reattach().await.expect("reattach");
    let _ = redraws.next("initial UI redraw").await;
    session
        .client()
        .request("nvim_command", Value::Array(vec!["enew!".into()]))
        .await
        .expect("open a clean buffer");
    let accepted = session
        .client()
        .request("nvim_input", Value::Array(vec!["ihello<Esc>".into()]))
        .await
        .expect("nvim_input RPC");
    assert!(accepted.as_i64().is_some_and(|count| count > 0));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let buffer = session
        .client()
        .request("nvim_get_current_buf", Value::Array(Vec::new()))
        .await
        .expect("current buffer");
    let lines = session
        .client()
        .request(
            "nvim_buf_get_lines",
            Value::Array(vec![buffer, 0.into(), (-1).into(), false.into()]),
        )
        .await
        .expect("buffer lines");
    assert!(
        lines
            .as_array()
            .is_some_and(|items| items.iter().any(|line| line.as_str() == Some("hello"))),
        "buffer did not change: {lines:?}"
    );
    let (events, _) = redraws
        .until("grid_line containing hello", grid_contains_hello)
        .await;
    assert!(
        grid_contains(&events, "hello"),
        "grid never carried hello: {events:?}"
    );
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn resize_and_external_ui_overlays_are_real() {
    if !have_nvim() {
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (session, _) = start(&project, "overlays").await;
    let mut redraws = RedrawStream::new(session.subscribe());
    session.reattach().await.expect("reattach");
    let _ = redraws.next("initial UI redraw").await;
    session
        .client()
        .request("nvim_command", Value::Array(vec!["enew!".into()]))
        .await
        .expect("open a clean buffer");
    session
        .client()
        .request("nvim_command", Value::Array(vec!["set showmode".into()]))
        .await
        .expect("enable mode messages");
    session
        .resize(UiSize::new(30, 100).expect("valid size"))
        .await
        .expect("resize");
    let (resize, _) = redraws
        .until("grid_resize 100x30", grid_resize_100x30)
        .await;
    assert!(
        event(&resize, "grid_resize").is_some(),
        "resize lacks grid_resize: {resize:?}"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    input_notify(&session, ":").await;
    let (cmdline, _) = redraws
        .until("cmdline_show", |events| has_event(events, "cmdline_show"))
        .await;
    assert!(event(&cmdline, "cmdline_show").is_some());
    assert!(
        !grid_has_row(&cmdline, 29),
        "cmdline was drawn into grid: {cmdline:?}"
    );
    input_notify(&session, "echo \"x\"<CR>").await;
    let (message, _) = redraws
        .until("msg_show", |events| event(events, "msg_show").is_some())
        .await;
    assert!(event(&message, "msg_show").is_some());
    input_notify(&session, "i").await;
    let (mode, _) = redraws
        .until("msg_showmode", |events| has_event(events, "msg_showmode"))
        .await;
    assert!(event(&mode, "msg_showmode").is_some());
    input_notify(&session, "<Esc>").await;
    session.shutdown().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn multigrid_float_has_its_own_grid_and_position() {
    if !have_nvim() {
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (session, _) = start(&project, "multigrid-float").await;
    let mut redraws = RedrawStream::new(session.subscribe());
    session.reattach().await.expect("reattach");
    let _ = redraws.next("initial UI redraw").await;
    session.client().request("nvim_command", Value::Array(vec!["lua local b=vim.api.nvim_create_buf(false,true); vim.api.nvim_buf_set_lines(b,0,-1,false,{'multigrid-float'}); vim.api.nvim_open_win(b,true,{relative='editor',row=2,col=3,width=18,height=3,style='minimal'})".into()])).await.expect("open a floating window");
    let mut float_grid = None;
    let mut saw_float_grid_resize = false;
    let mut saw_float_position = false;
    let mut saw_main_grid_text = false;
    let mut resized_grids = Vec::new();
    for _ in 0..20 {
        let (events, _) = redraws.next("multigrid float events").await;
        if let Some(position) = event(&events, "win_float_pos") {
            float_grid = position.first().and_then(Value::as_i64);
            saw_float_position = float_grid.is_some();
        }
        if let Some(resize) = event(&events, "grid_resize") {
            if let Some(grid) = resize.first().and_then(Value::as_i64) {
                resized_grids.push(grid);
            }
        }
        if let Some(grid) = float_grid {
            saw_float_grid_resize |= resized_grids.contains(&grid);
            saw_main_grid_text |= grid_line_contains(&events, 1, "multigrid-float");
        }
        if saw_float_position && saw_float_grid_resize && has_event(&events, "flush") {
            break;
        }
    }
    let grid = float_grid.expect("floating window must have a separate grid");
    assert_ne!(grid, 1);
    assert!(saw_float_grid_resize);
    assert!(saw_float_position);
    assert!(!saw_main_grid_text);
    session.shutdown().await;
}

fn grid_line_contains(events: &[Value], grid: i64, text: &str) -> bool {
    event(events, "grid_line").is_some_and(|args| {
        args.first().and_then(Value::as_i64) == Some(grid)
            && args
                .get(3)
                .is_some_and(|cells| format!("{cells:?}").contains(text))
    })
}
