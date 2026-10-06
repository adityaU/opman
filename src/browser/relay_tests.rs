use super::*;

#[test]
fn backoff_doubles_from_one_second_to_thirty() {
    let mut backoff = Backoff::default();
    let delays: Vec<u64> = (0..7).map(|_| backoff.next_delay().as_secs()).collect();
    assert_eq!(delays, vec![1, 2, 4, 8, 16, 30, 30]);
}

#[test]
fn a_healthy_link_resets_the_backoff() {
    let mut backoff = Backoff::default();
    backoff.next_delay();
    backoff.next_delay();
    backoff.reset();
    assert_eq!(backoff.next_delay(), MIN_BACKOFF);
}

type TestSocket = tokio_tungstenite::WebSocketStream<tokio::io::DuplexStream>;

#[tokio::test]
async fn a_server_mode_pool_refuses_to_relay() {
    let pool = BrowserPool::new(reqwest::Client::new());
    let (near, _far) = tokio::io::duplex(64);
    let socket: TestSocket = tokio_tungstenite::WebSocketStream::from_raw_socket(
        near,
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await;
    assert!(matches!(
        run_device_link(&pool, socket).await,
        Err(RelayError::NotDeviceMode)
    ));
}

#[tokio::test]
async fn the_loop_refuses_at_once_outside_device_mode() {
    let pool = BrowserPool::new(reqwest::Client::new());
    let connect = || async { Err::<TestSocket, _>(anyhow::anyhow!("never dialled")) };
    let outcome = run_device_link_loop(pool, connect, std::future::pending()).await;
    assert!(matches!(outcome, Err(RelayError::NotDeviceMode)));
}

#[tokio::test(start_paused = true)]
async fn the_loop_retries_with_backoff_until_shut_down() {
    let pool = BrowserPool::with_mode(reqwest::Client::new(), BrowserMode::Device);
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = attempts.clone();
    let connect = move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async { Err::<TestSocket, _>(anyhow::anyhow!("remote is down")) }
    };
    // 1 + 2 + 4 seconds of backoff fit before an 8s shutdown: four attempts.
    let shutdown = tokio::time::sleep(Duration::from_secs(8));
    let outcome = run_device_link_loop(pool, connect, shutdown).await;
    assert!(outcome.is_ok());
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 4);
}
