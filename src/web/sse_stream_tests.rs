//! Generated tests that DRIVE the SSE stream bodies in `web/sse.rs`.
//!
//! The render helpers and pre-stream guards are covered in `sse_tests.rs`. Here
//! we actually poll each stream's response body (with a 200-300ms per-frame
//! timeout) after pushing an event through the relevant broadcast channel, so
//! the `async_stream!` loop bodies (initial heartbeat, `recv` → render → yield,
//! and the `Lagged` refresh arms) execute.

use super::*;
use crate::web::test_support::test_server_state;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::StreamExt;
use std::time::Duration;

fn q() -> Query<SseTokenQuery> {
    Query(SseTokenQuery {
        token: None,
        id: None,
        replay: Replay::No,
    })
}

/// Poll up to `max` data frames off an SSE response body, concatenating the raw
/// bytes into a String. Stops early on timeout / stream end.
async fn collect_frames(body: axum::body::Body, max: usize, per_frame_ms: u64) -> String {
    let mut ds = body.into_data_stream();
    let mut out = String::new();
    for _ in 0..max {
        match tokio::time::timeout(Duration::from_millis(per_frame_ms), ds.next()).await {
            Ok(Some(Ok(bytes))) => out.push_str(&String::from_utf8_lossy(&bytes)),
            _ => break,
        }
    }
    out
}

// ── events_stream ───────────────────────────────────────────────────

#[tokio::test]
async fn events_stream_emits_heartbeat_then_rendered_event() {
    let state = test_server_state();
    let etx = state.event_tx.clone();
    let resp = events_stream(State(state), HeaderMap::new(), q())
        .await
        .unwrap()
        .into_response();
    // Buffered on the already-subscribed receiver; delivered once the loop runs.
    etx.send(WebEvent::StateChanged).unwrap();
    etx.send(WebEvent::SessionBusy {
        session_id: "s1".into(),
    })
    .unwrap();
    let s = collect_frames(resp.into_body(), 4, 300).await;
    assert!(s.contains("heartbeat"), "missing initial heartbeat: {s:?}");
    assert!(
        s.contains("state_changed") || s.contains("session_busy"),
        "no rendered event: {s:?}"
    );
}

#[tokio::test]
async fn events_stream_lagged_triggers_state_changed() {
    let state = test_server_state();
    let etx = state.event_tx.clone();
    let resp = events_stream(State(state), HeaderMap::new(), q())
        .await
        .unwrap()
        .into_response();
    // Overflow the 256-capacity broadcast buffer before the stream is polled so
    // the first `recv` returns `Lagged` → the loop yields a state_changed.
    for _ in 0..400 {
        let _ = etx.send(WebEvent::StateChanged);
    }
    let s = collect_frames(resp.into_body(), 4, 300).await;
    assert!(s.contains("heartbeat"));
    assert!(s.contains("state_changed"));
}

// ── editor_events_stream ────────────────────────────────────────────

#[tokio::test]
async fn editor_events_stream_emits_file_changed() {
    let state = test_server_state();
    let etx = state.editor_tx.clone();
    let resp = editor_events_stream(State(state), HeaderMap::new(), q())
        .await
        .unwrap()
        .into_response();
    etx.send(EditorEvent::FileChanged {
        path: "a.rs".into(),
        source: "ai_edit".into(),
    })
    .unwrap();
    let s = collect_frames(resp.into_body(), 3, 300).await;
    assert!(s.contains("heartbeat"));
    assert!(
        s.contains("file_changed"),
        "expected file_changed frame: {s:?}"
    );
}

#[tokio::test]
async fn editor_events_stream_lagged_triggers_refresh() {
    let state = test_server_state();
    let etx = state.editor_tx.clone();
    let resp = editor_events_stream(State(state), HeaderMap::new(), q())
        .await
        .unwrap()
        .into_response();
    // editor_tx capacity is 64 → overflow it to force a Lagged → "refresh".
    for _ in 0..200 {
        let _ = etx.send(EditorEvent::FileChanged {
            path: "x".into(),
            source: "web_save".into(),
        });
    }
    let s = collect_frames(resp.into_body(), 3, 300).await;
    assert!(
        s.contains("refresh"),
        "expected refresh frame on lag: {s:?}"
    );
}

// ── session_events_stream ───────────────────────────────────────────

#[tokio::test]
async fn session_events_stream_forwards_opencode_event() {
    let state = test_server_state();
    let rtx = state.raw_sse_tx.clone();
    let resp = session_events_stream(
        State(state),
        HeaderMap::new(),
        Query(SessionSseQuery {
            token: None,
            project_dir: Some("/p".into()),
        }),
    )
    .await
    .unwrap()
    .into_response();
    rtx.send(r#"{"type":"session.created"}"#.to_string())
        .unwrap();
    let s = collect_frames(resp.into_body(), 3, 300).await;
    assert!(s.contains("heartbeat"));
    assert!(s.contains("opencode"), "expected opencode frame: {s:?}");
}

#[tokio::test]
async fn session_events_stream_lagged_emits_lagged_frame() {
    let state = test_server_state();
    let rtx = state.raw_sse_tx.clone();
    let resp = session_events_stream(
        State(state),
        HeaderMap::new(),
        Query(SessionSseQuery {
            token: None,
            project_dir: None,
        }),
    )
    .await
    .unwrap()
    .into_response();
    // raw_sse_tx capacity is 256 → overflow to force Lagged → "lagged" frame.
    for i in 0..400 {
        let _ = rtx.send(format!("evt{i}"));
    }
    let s = collect_frames(resp.into_body(), 3, 300).await;
    assert!(s.contains("lagged"), "expected lagged frame: {s:?}");
}

// ── system_stats_stream (authorized path spawns the sampling thread) ─

#[tokio::test]
async fn system_stats_stream_yields_a_sample() {
    let state = test_server_state();
    let resp = system_stats_stream(State(state), HeaderMap::new(), q())
        .await
        .unwrap()
        .into_response();
    // The blocking thread seeds CPU for ~500ms before the first send; allow up
    // to 2s for one `system_stats` frame, then drop the stream (thread exits).
    let s = collect_frames(resp.into_body(), 1, 2000).await;
    assert!(
        s.contains("system_stats"),
        "expected a system_stats frame: {s:?}"
    );
}

// ── terminal_stream replay ──────────────────────────────────────────

/// Spawn a real PTY whose program prints `marker` and then idles, and return a
/// server state wired to it. Idling matters: a shell that exits is reaped, and
/// `get_output` would then have nothing to hand the stream.
async fn state_with_marker_pty(
    dir: &std::path::Path,
    marker: &str,
) -> (ServerState, crate::web::pty_manager::RawOutputBuffer) {
    use crate::web::pty_manager::pty_test_support::write_fake_bin;
    let script = format!("echo {marker}; while true; do sleep 1; done");
    let sh = write_fake_bin(dir, "replayshell", &script);
    std::env::set_var("SHELL", sh.display().to_string());

    let mut state = test_server_state();
    state.pty_mgr = crate::web::pty_manager::start_web_pty_manager();
    let buf = state
        .pty_mgr
        .spawn_shell("replay-term".into(), 24, 80, dir.to_path_buf())
        .await
        .expect("fake shell spawns");
    (state, buf)
}

fn term_query(replay: Replay) -> Query<SseTokenQuery> {
    Query(SseTokenQuery {
        token: None,
        id: Some("replay-term".into()),
        replay,
    })
}

/// Wait until the PTY reader thread has produced output, up to ~2s.
async fn await_output(buf: &crate::web::pty_manager::RawOutputBuffer) {
    for _ in 0..40 {
        if buf.dirty.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn terminal_stream_replays_scrollback_to_a_reattaching_reader() {
    let _g = crate::web::pty_manager::pty_test_support::env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (state, buf) = state_with_marker_pty(dir.path(), "MARKER_42").await;
    await_output(&buf).await;

    // First reader consumes the marker, exactly as a live tab would.
    let first = terminal_stream(
        State(state.clone()),
        HeaderMap::new(),
        term_query(Replay::No),
    )
    .await
    .expect("stream opens")
    .into_response();
    let live = collect_frames(first.into_body(), 2, 500).await;
    assert!(
        decoded_frames(&live).contains("MARKER_42"),
        "live: {live:?}"
    );
    drop(live);

    // A reload re-attaches: the bytes are already drained, so only replay can
    // bring them back.
    let second = terminal_stream(State(state), HeaderMap::new(), term_query(Replay::Yes))
        .await
        .expect("stream opens")
        .into_response();
    let replayed = collect_frames(second.into_body(), 1, 500).await;
    assert!(
        decoded_frames(&replayed).contains("MARKER_42"),
        "replay: {replayed:?}"
    );
}

#[tokio::test]
async fn terminal_stream_without_replay_starts_blank() {
    let _g = crate::web::pty_manager::pty_test_support::env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (state, buf) = state_with_marker_pty(dir.path(), "MARKER_99").await;
    await_output(&buf).await;

    let first = terminal_stream(
        State(state.clone()),
        HeaderMap::new(),
        term_query(Replay::No),
    )
    .await
    .expect("stream opens")
    .into_response();
    let _ = collect_frames(first.into_body(), 2, 500).await;

    let second = terminal_stream(State(state), HeaderMap::new(), term_query(Replay::No))
        .await
        .expect("stream opens")
        .into_response();
    let quiet = collect_frames(second.into_body(), 1, 300).await;
    assert!(
        !decoded_frames(&quiet).contains("MARKER_99"),
        "a fresh spawn must not repaint history: {quiet:?}"
    );
}

/// Concatenate the base64 payloads of every `data:` line back into text.
fn decoded_frames(raw: &str) -> String {
    raw.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .filter_map(|d| BASE64.decode(d.trim()).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .collect()
}
