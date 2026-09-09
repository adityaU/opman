//! What a pane shows and reads, against a real browser: how many pixels a frame carries,
//! and what counts as text on a page.
//!
//! Split from `live_tests.rs` when that file outgrew the size rule. The fixture page, the
//! server serving it and the throwaway pool all live there and are shared, so both files
//! describe the same page.

use super::live_tests::fixture_pool;

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

/// The failure that made reads useless on a real site: `innerText` degrades to
/// `textContent` on a page that is not being rendered, and stylesheet bodies came back
/// instead of prose. The extractor must never emit them, rendered or not.
#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn stylesheets_and_scripts_are_never_read_as_text() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    let text = pane.tab().read_text(None).await.expect("text extraction");

    assert!(
        !text.text.contains("color:red"),
        "css leaked: {}",
        text.text
    );
    assert!(
        !text.text.contains("window.noise"),
        "script leaked: {}",
        text.text
    );
    assert!(
        !text.text.contains("Hidden link"),
        "a hidden element leaked: {}",
        text.text
    );
    // Block boundaries survive, so the model does not read "DocsAPIWelcome".
    assert!(
        text.text.contains("Welcome\n"),
        "headings should end a line: {:?}",
        text.text
    );

    pool.shutdown().await;
}

/// The pane looked soft next to the rest of the app because a frame carried one pixel per
/// CSS pixel however dense the display was. Sharpness is not the emulated device scale
/// factor — that only changes what the page believes — it is how wide the frame is copied.
#[tokio::test]
#[ignore = "launches a real Chromium"]
async fn a_retina_pane_is_captured_at_twice_the_pixels_of_a_plain_one() {
    let (pool, _profile, pane_id) = fixture_pool().await;
    let pane = pool.get(pane_id).await.expect("the pane is open");
    let _viewer = pane.tab().screencast().viewer().await;

    assert_eq!(
        frame_width(&pane, Some(2.0)).await,
        1200,
        "a 600px pane on a 2x display"
    );
    assert_eq!(
        frame_width(&pane, None).await,
        600,
        "the same pane on a plain one"
    );

    pool.shutdown().await;
}

/// Resize the pane and report how wide the next frame actually is.
async fn frame_width(pane: &super::pane::Pane, ratio: Option<f64>) -> u32 {
    use super::types::Viewport;

    pane.tab()
        .resize(Viewport::new(600, 400, ratio))
        .await
        .expect("resize");

    // A resize restarts the stream, so the frame in hand is the old size; read on until
    // one arrives at the new one, or give up rather than hang the suite.
    let expected = (600.0 * ratio.unwrap_or(1.0)) as u32;
    let mut seen = 0;
    let mut last = 0;
    for _ in 0..40 {
        let Some((frame, version)) = pane.tab().screencast().next_after(seen).await else {
            continue;
        };
        seen = version;
        let Some(width) = jpeg_width(&frame) else {
            continue;
        };
        last = width;
        if width == expected {
            return width;
        }
    }
    // Whatever it settled on, so a failure says what was captured instead of just "0".
    last
}

/// A JPEG's width, from its start-of-frame header. Cheaper than pulling in a decoder for
/// the one number this test needs.
fn jpeg_width(base64_frame: &str) -> Option<u32> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_frame)
        .ok()?;
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        // SOF0..SOF15, minus the three markers in that range that are not frame headers.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            return Some(u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]).into());
        }
        i += 2 + length;
    }
    None
}
