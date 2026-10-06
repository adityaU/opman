//! A device link driving the pool, with a fake browser on the far end of the channels.

use std::time::Duration;

use serde_json::json;

use super::fake_device::{self, CallLog};
use super::mode::OFFSCREEN_ORIGIN;
use super::{BrowserPool, LinkState, Opened};

/// Attach a link and serve it with the fake browser.
async fn linked_pool() -> (BrowserPool, super::LinkId, CallLog) {
    let pool = BrowserPool::new(reqwest::Client::new());
    let link = pool.attach_link().await;
    let log = CallLog::default();
    tokio::spawn(fake_device::serve(
        link.to_device,
        link.from_device,
        log.clone(),
    ));
    (pool, link.id, log)
}

#[tokio::test]
async fn a_pane_opens_over_the_link_instead_of_launching() {
    let (pool, _id, log) = linked_pool().await;

    let (_, opened) = pool
        .open("proj:/x", "/x")
        .await
        .expect("opens over the link");
    assert_eq!(opened, Opened::Created);

    let methods = fake_device::methods(&log).await;
    assert_eq!(
        methods.first().map(String::as_str),
        Some("Target.createTarget")
    );
    assert!(methods.iter().any(|m| m == "Target.attachToTarget"));
    assert!(methods
        .iter()
        .any(|m| m == "Emulation.setDeviceMetricsOverride"));
}

#[tokio::test]
async fn a_linked_pane_window_is_parked_off_screen() {
    let (pool, _id, log) = linked_pool().await;
    pool.open("proj:/x", "/x").await.expect("opens");

    let create = fake_device::first_call(&log, "Target.createTarget")
        .await
        .expect("a target was created");
    assert_eq!(create["params"]["left"], json!(OFFSCREEN_ORIGIN));
    let bounds = fake_device::first_call(&log, "Browser.setWindowBounds")
        .await
        .expect("the window was moved");
    assert_eq!(bounds["params"]["windowId"], json!(41));
    assert_eq!(bounds["params"]["bounds"]["top"], json!(OFFSCREEN_ORIGIN));
}

#[tokio::test]
async fn status_reports_the_link() {
    let pool = BrowserPool::new(reqwest::Client::new());
    assert_eq!(pool.status().await.device_link, LinkState::Detached);
    let link = pool.attach_link().await;
    assert_eq!(pool.status().await.device_link, LinkState::Attached);
    pool.detach_link(link.id).await;
    assert_eq!(pool.status().await.device_link, LinkState::Detached);
}

#[tokio::test]
async fn a_dropped_link_takes_its_panes_with_it() {
    let (pool, id, _log) = linked_pool().await;
    pool.open("proj:/x", "/x").await.expect("opens");
    assert_eq!(pool.list().await.len(), 1);

    pool.detach_link(id).await;
    assert!(pool.list().await.is_empty());
    assert!(pool.get("proj:/x").await.is_none());
}

#[tokio::test]
async fn the_newest_link_wins_and_the_old_one_is_told() {
    let pool = BrowserPool::new(reqwest::Client::new());
    let first = pool.attach_link().await;
    let second = pool.attach_link().await;

    let told = tokio::time::timeout(Duration::from_secs(1), first.superseded).await;
    assert!(told.is_ok(), "the replaced link's transport is signalled");

    // The stale transport ending must not detach its successor.
    pool.detach_link(first.id).await;
    assert_eq!(pool.status().await.device_link, LinkState::Attached);
    pool.detach_link(second.id).await;
    assert_eq!(pool.status().await.device_link, LinkState::Detached);
}

#[tokio::test]
async fn a_pane_measures_the_surface_it_renders_on() {
    let (pool, _id, _log) = linked_pool().await;
    let (pane, _) = pool.open("proj:/x", "/x").await.expect("opens");
    // The fake reports a one-to-one surface (no forced scale on a device), so a retina
    // pane is held to 1x rather than handed a scale its frames do not have.
    let applied = pane
        .tab()
        .resize(super::Viewport::new(800, 600, Some(2.0)))
        .await
        .expect("resizes");
    assert_eq!(applied.scale(), 1.0);
}
