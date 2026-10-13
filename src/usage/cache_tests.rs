use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use super::*;

fn counting(calls: &Arc<AtomicUsize>, delay: Duration) -> impl Future<Output = usize> {
    let calls = calls.clone();
    async move {
        tokio::time::sleep(delay).await;
        calls.fetch_add(1, Ordering::SeqCst) + 1
    }
}

#[tokio::test(start_paused = true)]
async fn fresh_value_is_reused_until_the_ttl() {
    let cache = Cached::new(|_| Duration::from_secs(60));
    let calls = Arc::new(AtomicUsize::new(0));
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        1
    );
    tokio::time::advance(Duration::from_secs(59)).await;
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        1
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        2
    );
}

#[tokio::test(start_paused = true)]
async fn force_bypasses_a_fresh_value() {
    let cache = Cached::new(|_| Duration::from_secs(60));
    let calls = Arc::new(AtomicUsize::new(0));
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        1
    );
    assert_eq!(
        cache.get(true, || counting(&calls, Duration::ZERO)).await,
        2
    );
}

#[tokio::test(start_paused = true)]
async fn concurrent_callers_share_one_fetch() {
    let cache = Cached::new(|_| Duration::from_secs(60));
    let calls = Arc::new(AtomicUsize::new(0));
    let slow = Duration::from_secs(3);
    let (a, b, c) = tokio::join!(
        cache.get(false, || counting(&calls, slow)),
        cache.get(true, || counting(&calls, slow)),
        cache.get(false, || counting(&calls, slow)),
    );
    assert_eq!((a, b, c), (1, 1, 1));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// A failed read must not hide usage for the full minute: odd values stand in for errors
/// here and get a short life, even ones a long one.
#[tokio::test(start_paused = true)]
async fn each_value_decides_its_own_freshness() {
    let cache: Cached<usize> = Cached::new(|value| match value % 2 {
        1 => Duration::from_secs(10),
        _ => Duration::from_secs(60),
    });
    let calls = Arc::new(AtomicUsize::new(0));
    // First fetch returns 1: an "error", fresh for 10s only.
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        1
    );
    tokio::time::advance(Duration::from_secs(11)).await;
    // Refetched: 2 is a good value, now fresh for a minute.
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        2
    );
    tokio::time::advance(Duration::from_secs(30)).await;
    assert_eq!(
        cache.get(false, || counting(&calls, Duration::ZERO)).await,
        2
    );
}
