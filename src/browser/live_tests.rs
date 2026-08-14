//! End-to-end tests against a real Chromium. Ignored by default because they need the
//! browser on the host and a few seconds each:
//!
//! ```text
//! cargo test browser::live -- --ignored --test-threads=1
//! ```
//!
//! These are the tests that actually prove the token claim: the fixture page below is
//! ~4 KB of HTML and its outline is a few hundred bytes.

use super::pool::BrowserPool;
use super::types::SnapshotOptions;

/// Browsers are per project; the live tests only need a stable name for one.
const PROJECT: &str = "/tmp/opman-live-test-project";

/// A pool on a throwaway profile. Chromium locks a profile directory exclusively, so
/// tests sharing one would serialise behind each other — and a test that panics before
/// `shutdown` would block every test after it.
fn pool() -> (BrowserPool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temp profile directory");
    let pool = BrowserPool::with_profile(reqwest::Client::new(), dir.path().join("profile"));
    (pool, dir)
}

/// A page with the shapes that matter: a landmark, a heading, links, a labelled input, a
/// checkbox, prose, and a pile of markup that must NOT reach the outline.
const FIXTURE: &str = r#"<html><head><title>Fixture</title><style>.a{color:red}</style></head>
<body>
  <nav aria-label="Primary"><a href="/docs">Docs</a><a href="/api">API</a></nav>
  <main>
    <h1>Welcome</h1>
    <p>The quick brown fox jumps over the lazy dog.</p>
    <form>
      <label for="q">Search</label><input id="q" type="text" placeholder="Type here">
      <input id="c" type="checkbox" checked><label for="c">Remember me</label>
      <button type="submit">Go</button>
    </form>
    <div><div><div><span>deeply nested filler</span></div></div></div>
    <script>window.noise = 1;</script>
    <div hidden><a href="/hidden">Hidden link</a></div>
  </main>
</body></html>"#;

/// Serve one page on an ephemeral loopback port and return its URL.
///
/// A real HTTP origin rather than a `data:` URL, because `data:` is deliberately not
/// navigable (see `normalize_url`) — and because serving it for real is what exercises
/// the framing probe alongside the page load.
async fn serve(body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();

    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut socket, _)) = listener.accept().await {
            // One task per connection. Chromium preconnects sockets it may never send a
            // request on, so serving connections in the accept loop would park on a silent
            // preconnect and never reach the real request — which looks exactly like a
            // hung navigation.
            tokio::spawn(async move {
                let mut discard = [0_u8; 2048];
                let read = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    socket.read(&mut discard),
                )
                .await;
                if !matches!(read, Ok(Ok(n)) if n > 0) {
                    return;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    format!("http://127.0.0.1:{port}/")
}

async fn fixture_pool() -> (BrowserPool, tempfile::TempDir, &'static str) {
    let url = serve(FIXTURE).await;
    let (pool, dir) = pool();
    let pane = "live-test-pane";
    pool.navigate(pane, PROJECT, &url)
        .await
        .expect("the fixture page loads");
    (pool, dir, pane)
}

#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn the_outline_describes_the_page_without_its_markup() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    let page = pane
        .tab()
        .snapshot(SnapshotOptions::default())
        .await
        .expect("the page snapshots");

    assert_eq!(page.title, "Fixture");
    let outline = &page.outline;

    // Structure and controls are present, each actionable one with a ref.
    assert!(outline.contains("h1 \"Welcome\""), "{outline}");
    assert!(outline.contains("link \"Docs\""), "{outline}");
    assert!(outline.contains("button \"Go\""), "{outline}");
    assert!(outline.contains("[ref=e"), "{outline}");
    assert!(
        outline.contains("checked"),
        "checkbox state is worth a token"
    );
    assert!(outline.contains("quick brown fox"), "prose survives");

    // Markup, scripts, styles and hidden nodes do not.
    assert!(!outline.contains('<'), "no tags reach the model: {outline}");
    assert!(!outline.contains("window.noise"), "{outline}");
    assert!(!outline.contains("color:red"), "{outline}");
    assert!(!outline.contains("Hidden link"), "{outline}");

    // The whole point: the outline is a fraction of the source.
    assert!(
        outline.len() < FIXTURE.len() / 2,
        "outline was {} bytes for a {} byte page",
        outline.len(),
        FIXTURE.len()
    );

    pool.shutdown().await;
}

#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn a_ref_from_the_outline_can_be_typed_into_and_read_back() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    let page = pane
        .tab()
        .snapshot(SnapshotOptions::default())
        .await
        .expect("snapshot");

    // Find the textbox's ref the way the model would: by reading the outline.
    let reference = page
        .outline
        .lines()
        .find(|line| line.contains("textbox"))
        .and_then(|line| line.split("[ref=").nth(1))
        .and_then(|rest| rest.split(']').next())
        .expect("the search box is in the outline")
        .to_owned();

    pane.tab()
        .type_ref(&reference, "hello world", false)
        .await
        .expect("typing succeeds");

    let after = pane
        .tab()
        .snapshot(SnapshotOptions::default())
        .await
        .expect("re-snapshot");
    assert!(
        after.outline.contains("value=\"hello world\""),
        "the typed value should be visible in the next outline: {}",
        after.outline
    );

    pool.shutdown().await;
}

#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn a_stale_ref_is_refused_rather_than_hitting_the_wrong_element() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    pane.tab()
        .snapshot(SnapshotOptions::default())
        .await
        .expect("snapshot");

    let error = pane
        .tab()
        .click_ref("e9999")
        .await
        .expect_err("a ref that was never issued must fail");
    assert!(
        error.to_string().contains("re-snapshot"),
        "the error should tell the model what to do: {error}"
    );

    pool.shutdown().await;
}

#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn readable_text_drops_the_navigation() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    let text = pane.tab().read_text(None).await.expect("text extraction");

    assert!(text.text.contains("quick brown fox"), "{}", text.text);
    assert_eq!(text.title, "Fixture");

    pool.shutdown().await;
}

#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn panes_are_independent_tabs_on_one_browser() {
    let first = serve(FIXTURE).await;
    let second =
        serve("<html><head><title>Other</title></head><body><h1>Other</h1></body></html>").await;

    let (pool, _profile) = pool();
    pool.navigate("pane-a", "/repo/a", &first)
        .await
        .expect("pane a loads");
    pool.navigate("pane-b", "/repo/b", &second)
        .await
        .expect("pane b loads");

    let listed = pool.list().await;
    assert_eq!(listed.len(), 2);
    let titles: Vec<_> = listed.iter().map(|pane| pane.title.as_str()).collect();
    assert!(titles.contains(&"Fixture"), "{titles:?}");
    assert!(titles.contains(&"Other"), "{titles:?}");

    // Closing one leaves the other alone — panes must not share a target.
    pool.close("pane-a").await;
    assert_eq!(pool.list().await.len(), 1);

    pool.shutdown().await;
}

/// The restart story, end to end: a browser whose parent was killed keeps the profile
/// lock, and the next opman must join it rather than die on "chromium exited before
/// printing a DevTools endpoint". Then, once that browser is gone, its leftover lock must
/// not block a fresh launch either.
#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn a_browser_outliving_its_parent_is_adopted_and_its_lock_later_cleared() {
    use super::chrome::Chrome;
    use super::profile::Owner;

    let dir = tempfile::tempdir().expect("a temp profile directory");
    let profile = dir.path().join("profile");
    std::fs::create_dir_all(&profile).expect("the profile directory");

    let orphan = Chrome::launch(&profile).await.expect("the first launch");
    let endpoint = orphan.ws_url().to_owned();
    // Exactly what a SIGKILLed parent leaves: a live browser and no handle to reap it.
    std::mem::forget(orphan);

    let mut adopted = Chrome::launch(&profile).await.expect("the second launch");
    assert_eq!(adopted.ws_url(), endpoint, "it should join, not relaunch");
    assert!(adopted.is_alive());

    let Owner::Live { pid, .. } = super::profile::owner(&profile) else {
        panic!("the profile should read as held by a live browser");
    };
    // Adopting must not have taken ownership: ending the pool leaves the browser up.
    adopted.shutdown().await;
    assert!(
        super::profile::pid_alive(pid),
        "an adopted browser is not ours to kill"
    );

    let _ = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status();
    for _ in 0..50 {
        if !super::profile::pid_alive(pid) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let fresh = Chrome::launch(&profile)
        .await
        .expect("the stale lock is cleared");
    assert_ne!(
        fresh.ws_url(),
        endpoint,
        "the dead browser must not be adopted"
    );
    fresh.shutdown().await;
}
