//! Regression tests for runner-owned session mutations.

use super::*;
use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    routing::patch,
    Json, Router,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn serve(app: Router) -> Result<String, Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(format!("http://{address}"))
}

#[tokio::test]
async fn http_runner_mutations_use_the_runner_endpoint_and_directory(
) -> Result<(), Box<dyn std::error::Error>> {
    let seen = Arc::new(Mutex::new(Vec::<(String, String, Option<Value>)>::new()));
    let patch_seen = Arc::clone(&seen);
    let delete_seen = Arc::clone(&seen);
    let app = Router::new().route(
        "/session/{id}",
        patch(
            move |Path(id): Path<String>, headers: HeaderMap, Json(body): Json<Value>| {
                let seen = Arc::clone(&patch_seen);
                async move {
                    let directory = headers
                        .get("x-opencode-directory")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    seen.lock().await.push((id, directory, Some(body)));
                    Json(serde_json::json!({ "ok": true }))
                }
            },
        )
        .delete(move |Path(id): Path<String>, headers: HeaderMap| {
            let seen = Arc::clone(&delete_seen);
            async move {
                let directory = headers
                    .get("x-opencode-directory")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                seen.lock().await.push((id, directory, None));
                StatusCode::NO_CONTENT
            }
        }),
    );
    let runner = HttpRunner::new(
        RunnerKind::Claude,
        serve(app).await?,
        reqwest::Client::new(),
    );

    assert!(runner.rename("session-1", "Renamed", "/project").await?);
    assert!(runner.delete("session-1", "/project").await?);

    let seen = seen.lock().await;
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].0, "session-1");
    assert_eq!(seen[0].1, "/project");
    assert_eq!(
        seen[0].2.as_ref().and_then(|body| body["title"].as_str()),
        Some("Renamed")
    );
    assert_eq!(
        seen[1],
        ("session-1".to_string(), "/project".to_string(), None)
    );
    Ok(())
}
