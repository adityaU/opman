//! Readiness tests for the lazy runner's startup contract.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use super::{LazyContext, LazyRunner, LazyStart};
use crate::runner::{Runner, RunnerKind, RunnerRegistry, RunnerStartOutcome};

#[tokio::test]
async fn started_runner_answers_before_startup_resolves() {
    let ready = Arc::new(AtomicBool::new(false));
    let health_ready = ready.clone();
    let message_ready = ready.clone();
    let app = Router::new()
        .route(
            "/health",
            get(move || {
                let ready = health_ready.clone();
                async move {
                    ready.store(true, Ordering::SeqCst);
                    "ok"
                }
            }),
        )
        .route(
            "/session/s1/message",
            get(move || {
                let ready = message_ready.clone();
                async move {
                    if ready.load(Ordering::SeqCst) {
                        (StatusCode::OK, Json(json!(["from the real runner"])))
                    } else {
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({ "error": "runner is not ready" })),
                        )
                    }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let ctx = LazyContext::new();
    let runner_url = base_url.clone();
    let lazy = Arc::new(LazyRunner::new(
        RunnerKind::Opencode,
        ctx.clone(),
        move || {
            let runner_url = runner_url.clone();
            async move {
                Ok(LazyStart {
                    runner: Arc::new(crate::runner::HttpRunner::new(
                        RunnerKind::Opencode,
                        runner_url,
                        reqwest::Client::new(),
                    )) as Arc<dyn Runner>,
                    handle: None,
                    engine: None,
                    base_url: None,
                })
            }
        },
    ));
    let mut runners = std::collections::HashMap::new();
    runners.insert(RunnerKind::Opencode, lazy as Arc<dyn Runner>);
    let registry = Arc::new(RunnerRegistry::new(RunnerKind::Opencode, runners));
    ctx.set_registry(&registry);

    let outcome = registry
        .ensure_started(&RunnerKind::Opencode)
        .await
        .unwrap();
    assert_eq!(outcome, RunnerStartOutcome::Started);
    assert!(ready.load(Ordering::SeqCst));
    assert_eq!(
        registry.messages("s1", "/p").await.unwrap(),
        json!(["from the real runner"])
    );
}

/// opencode binds its port before it serves: a probe sent in that window is never
/// answered. Readiness must retry with short probes rather than let one hang until the
/// deadline.
#[tokio::test]
async fn readiness_survives_a_probe_the_server_never_answers() {
    let probes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = probes.clone();
    let app = Router::new().route(
        "/health",
        get(move || {
            let seen = seen.clone();
            async move {
                if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                    std::future::pending::<()>().await;
                }
                "ok"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let runner =
        crate::runner::HttpRunner::new(RunnerKind::Opencode, base_url, reqwest::Client::new());
    let started = tokio::time::Instant::now();
    Runner::wait_until_ready(&runner).await.unwrap();
    assert!(
        probes.load(Ordering::SeqCst) >= 2,
        "the hung probe must be retried"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "readiness must not wait out the whole deadline on one hung probe"
    );
}
