//! Coverage for the live-forward branch of `kanban_add_user_note`.
//!
//! When a task is running/launching with a session, the note POST spawns a
//! fire-and-forget message into that session and then broadcasts a delivery
//! Toast. The spawned task does NOT inherit the per-task base-URL override, so
//! it hits the (dead) process-global upstream and the POST errors — but the
//! whole spawn body still executes, and we prove that by awaiting its Toast.
use super::*;
use crate::web::types::default_board;
use crate::web::web_state::WebStateHandle;

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:9".to_string());
}

fn seed_board(h: &WebStateHandle, board_id: &str, project: &str) {
    h.db_for_test().insert_kanban_board(
        &default_board(board_id.into(), project.into()),
        "2026-01-01T00:00:00Z",
    );
}

async fn make_task(h: &WebStateHandle, board_id: &str, lane: &str) -> Task {
    h.create_kanban_task(CreateTaskRequest {
        board_id: board_id.into(),
        lane_id: lane.into(),
        title: "Task".into(),
        description: String::new(),
        tags: vec![],
        priority: "normal".into(),
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn add_user_note_running_forwards_and_broadcasts_toast() {
    ensure_base_url(); // the spawned forward reads the process-global base_url
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // running + a session id → the (true, Some(sid), Some(board)) forward fires.
    h.set_kanban_task_launch(&t.id, Some("sess-live".into()), None, None, "running")
        .await
        .unwrap();

    let mut rx = h.subscribe_events();
    let note = h.kanban_add_user_note(&t.id, "please rebase onto main").await.unwrap();
    assert_eq!(note.author, "user");
    assert_eq!(note.body, "please rebase onto main");
    assert_eq!(h.db_for_test().kanban_notes_for_task(&t.id).len(), 1);

    // The spawned forward always ends by broadcasting an info-level delivery
    // Toast; awaiting it proves the spawn body ran to completion.
    let saw = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match rx.recv().await {
                Ok(WebEvent::Toast { message, level }) if message.contains("Note delivered") => {
                    assert_eq!(level, "info");
                    break true;
                }
                Ok(_) => continue,
                Err(_) => break false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(saw, "expected the delivery Toast from the fire-and-forget forward");
}

#[tokio::test]
async fn add_user_note_idle_task_does_not_forward() {
    let h = WebStateHandle::new_test();
    seed_board(&h, "brd", "/p");
    let t = make_task(&h, "brd", "lane_todo").await;
    // idle run_state with a session id: `live` is false → no forward, no Toast.
    h.set_kanban_task_launch(&t.id, Some("sess-idle".into()), None, None, "idle")
        .await
        .unwrap();
    let mut rx = h.subscribe_events();
    let note = h.kanban_add_user_note(&t.id, "fyi").await.unwrap();
    assert_eq!(note.author, "user");
    // Only the KanbanTaskUpdated broadcast — never a delivery Toast.
    let mut saw_toast = false;
    while let Ok(ev) = rx.try_recv() {
        if let WebEvent::Toast { message, .. } = ev {
            if message.contains("Note delivered") {
                saw_toast = true;
            }
        }
    }
    assert!(!saw_toast, "idle task must not forward the note");
}

#[tokio::test]
async fn add_user_note_task_not_found() {
    let h = WebStateHandle::new_test();
    assert!(matches!(
        h.kanban_add_user_note("ghost", "x").await,
        Err(KanbanError::NotFound)
    ));
}
