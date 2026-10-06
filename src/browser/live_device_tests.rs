//! Device mode and the device-link relay against a real browser. Ignored by default:
//!
//! ```text
//! cargo test browser::live_device -- --ignored --test-threads=1
//! ```
//!
//! On a host without a desktop, device mode falls back to a virtual display (and says so),
//! which is still enough to prove parking, screencasting and relaying.

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use super::live_tests::serve;
use super::mode::{BrowserMode, OFFSCREEN_ORIGIN};
use super::pool::BrowserPool;

const PAGE: &str = "<html><head><title>Device</title></head><body><h1 id=n>0</h1>\
    <script>let n=0;setInterval(()=>document.getElementById('n').textContent=++n,100)</script>\
    </body></html>";

fn device_pool() -> (BrowserPool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temp profile directory");
    let pool = BrowserPool::with_mode_and_profile(
        reqwest::Client::new(),
        BrowserMode::Device,
        dir.path().join("device-profile"),
    );
    (pool, dir)
}

#[tokio::test]
#[ignore = "launches a real browser"]
async fn a_device_pane_is_parked_off_screen_and_still_paints() {
    let (pool, _profile) = device_pool();
    let url = serve(PAGE).await;
    pool.navigate("proj:/d", "/d", &url)
        .await
        .expect("navigates");
    let pane = pool.get("proj:/d").await.expect("the pane is open");

    let (cdp, target) = pane.tab().target();
    let window = cdp
        .call("Browser.getWindowForTarget", json!({ "targetId": target }))
        .await
        .expect("the window is reported");
    assert_eq!(window["bounds"]["left"], json!(OFFSCREEN_ORIGIN));
    assert_eq!(window["bounds"]["top"], json!(OFFSCREEN_ORIGIN));

    let screencast = pane.tab().screencast().clone();
    let _viewer = screencast.viewer().await;
    let mut seen = 0;
    let mut frames = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while frames < 3 && tokio::time::Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_secs(2), screencast.next_after(seen)).await;
        if let Ok(Some((_, version))) = next {
            seen = version;
            frames += 1;
        }
    }
    assert!(
        frames >= 3,
        "an off-screen window kept painting ({frames} frames)"
    );
    pool.shutdown().await;
}

/// The far end of a device link, as `/api/browser/device` runs it, over an in-memory
/// websocket.
async fn attach_over_socket(remote: BrowserPool, socket: WebSocketStream<tokio::io::DuplexStream>) {
    let mut link = remote.attach_link().await;
    let (mut sink, mut source) = socket.split();
    loop {
        tokio::select! {
            out = link.to_device.recv() => {
                let Some(text) = out else { break };
                if sink.send(Message::Text(text.into())).await.is_err() { break }
            }
            frame = source.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if link.from_device.send(text.as_str().to_owned()).await.is_err() { break }
                }
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
    remote.detach_link(link.id).await;
}

#[tokio::test]
#[ignore = "launches a real browser"]
async fn a_remote_drives_the_device_browser_through_the_relay() {
    let (device, _profile) = device_pool();
    let remote = BrowserPool::new(reqwest::Client::new());
    let (near, far) = tokio::io::duplex(1 << 20);
    let upstream = WebSocketStream::from_raw_socket(near, Role::Client, None).await;
    let server_side = WebSocketStream::from_raw_socket(far, Role::Server, None).await;

    tokio::spawn(attach_over_socket(remote.clone(), server_side));
    let relay_pool = device.clone();
    let relay = tokio::spawn(async move { super::run_device_link(&relay_pool, upstream).await });

    let url = serve(PAGE).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while remote.status().await.device_link != super::LinkState::Attached {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    remote
        .navigate("proj:/r", "/r", &url)
        .await
        .expect("navigates over the relay");
    let pane = remote
        .get("proj:/r")
        .await
        .expect("the remote pane is open");
    let page = pane
        .tab()
        .snapshot(Default::default())
        .await
        .expect("reads the page");
    assert_eq!(page.title, "Device");

    // The window lives in the device browser, parked like a local one.
    let (_, target) = pane.tab().target();
    let local = super::cdp::Cdp::connect(&device.local_endpoint().await.expect("endpoint"))
        .await
        .expect("a second client");
    let targets: Value = local
        .call("Target.getTargets", json!({}))
        .await
        .expect("targets");
    let ids: Vec<&str> = targets["targetInfos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|info| info["targetId"].as_str())
        .collect();
    assert!(
        ids.contains(&target),
        "the remote's tab is in the device browser"
    );

    relay.abort();
    pool_shutdown(device).await;
}

async fn pool_shutdown(pool: BrowserPool) {
    pool.shutdown().await;
}
