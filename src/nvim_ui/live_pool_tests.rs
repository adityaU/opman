use super::super::key::{SessionKey, UiSize};
use super::helpers::{fixture, grid_contains, have_nvim, input, lock, minimal_pool, RedrawStream};

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn process_death_removes_pool_session_and_registry_entry() {
    if !have_nvim() {
        eprintln!("skipping: nvim not installed");
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (pool, registry) = minimal_pool(&project, "death");
    let key = SessionKey::new(0, "death");
    let session = pool
        .ensure(key.clone(), project.path(), UiSize::default())
        .await
        .expect("ensure");
    let socket = session
        .socket_path()
        .to_str()
        .expect("socket path is UTF-8");
    std::process::Command::new("pkill")
        .args(["-TERM", "-f", socket])
        .status()
        .expect("kill Neovim child");
    for _ in 0..100 {
        if !session.is_alive() && !registry.read().await.contains_key(&(0, "death".into())) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        !session.is_alive(),
        "pool session did not observe process death"
    );
    assert!(!registry.read().await.contains_key(&(0, "death".into())));
    assert_eq!(pool.sweep(std::time::Duration::ZERO).await, 1);
    assert!(pool.get(&key).await.is_none());
    pool.shutdown_all().await;
}

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn ensure_attach_input_frames_reach_the_notification_stream() {
    if !have_nvim() {
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (pool, _) = minimal_pool(&project, "loop");
    let session = pool
        .ensure(
            SessionKey::new(0, "loop"),
            project.path(),
            UiSize::default(),
        )
        .await
        .expect("ensure");
    let mut redraws = RedrawStream::new(session.subscribe());
    session.reattach().await.expect("attach");
    let _ = redraws.next("initial UI redraw").await;
    session
        .client()
        .request("nvim_command", rmpv::Value::Array(vec!["enew!".into()]))
        .await
        .expect("open a clean buffer");
    input(&session, "ihello<Esc>").await;
    let mut saw_hello = false;
    for _ in 0..20 {
        let (events, _) = redraws.next("input redraw frame").await;
        saw_hello |= grid_contains(&events, "hello");
        if saw_hello {
            break;
        }
    }
    assert!(saw_hello, "notification stream saw no input redraw");
    pool.shutdown_all().await;
}
