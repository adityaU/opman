//! A stand-in remote opman for the proxy tests: password login minting per-epoch
//! tokens, an echo endpoint, a held-open SSE stream, and a WebSocket echo.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures::{SinkExt, StreamExt};
use tokio::sync::Notify;

use crate::remote::{NewServer, Registry, ServerId, ServerStore};
use crate::web::test_support::{start_mock_upstream, test_router, test_server_state};

pub const USER: &str = "u";
pub const PASS: &str = "p";

#[derive(Clone, Default)]
pub struct Fake {
    /// Bumping this invalidates every issued token, as a remote restart does.
    pub epoch: Arc<AtomicU64>,
    pub logins: Arc<AtomicU64>,
    /// The SSE stream holds its second event until this is notified.
    pub release: Arc<Notify>,
}

impl Fake {
    fn authorized(&self, headers: &HeaderMap) -> bool {
        let expected = format!("Bearer t{}", self.epoch.load(Ordering::SeqCst));
        headers.get(header::AUTHORIZATION).is_some_and(|v| v == expected.as_str())
    }

    pub fn rotate(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub fn logins(&self) -> u64 {
        self.logins.load(Ordering::SeqCst)
    }
}

async fn login(State(fake): State<Fake>, Json(body): Json<serde_json::Value>) -> Response {
    if body["username"] != USER || body["password"] != PASS {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    fake.logins.fetch_add(1, Ordering::SeqCst);
    let token = format!("t{}", fake.epoch.load(Ordering::SeqCst));
    Json(serde_json::json!({ "token": token })).into_response()
}

async fn verify(State(fake): State<Fake>, headers: HeaderMap) -> StatusCode {
    if fake.authorized(&headers) {
        return StatusCode::OK;
    }
    StatusCode::UNAUTHORIZED
}

async fn echo(State(fake): State<Fake>, request: Request) -> Response {
    if !fake.authorized(request.headers()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
    let reply = serde_json::json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query(),
        "cookie": parts.headers.get(header::COOKIE).is_some(),
        "x_test": parts.headers.get("x-test").and_then(|v| v.to_str().ok()),
        "body": String::from_utf8_lossy(&body),
    });
    (
        [(header::SET_COOKIE, "opman_token=remote; Path=/")],
        Json(reply),
    )
        .into_response()
}

async fn sse(State(fake): State<Fake>, headers: HeaderMap) -> Response {
    if !fake.authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let stream = async_stream::stream! {
        yield Ok::<_, std::io::Error>(Bytes::from_static(b"data: one\n\n"));
        fake.release.notified().await;
        yield Ok(Bytes::from_static(b"data: two\n\n"));
    };
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/event-stream"),
    );
    response
}

async fn ws(State(fake): State<Fake>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    if !fake.authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    upgrade.on_upgrade(|socket| async move {
        let (mut tx, mut rx) = socket.split();
        while let Some(Ok(message)) = rx.next().await {
            if tx.send(message).await.is_err() {
                break;
            }
        }
    })
}

/// Serve the fake remote; returns its base URL.
pub async fn spawn(fake: Fake) -> String {
    let router = Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/verify", get(verify))
        .route("/api/echo/{*rest}", any(echo))
        .route("/api/sse", get(sse))
        .route("/api/ws", get(ws))
        .with_state(fake);
    start_mock_upstream(router).await
}

/// A served home with one remote (`remote`) pointing at a fresh fake.
pub struct Setup {
    pub fake: Fake,
    pub home: String,
    pub id: ServerId,
    pub registry: Registry,
}

pub async fn setup() -> Setup {
    let fake = Fake::default();
    let url = spawn(fake.clone()).await;
    let registry = Registry::new(ServerStore::memory());
    let snapshot = registry
        .add(NewServer {
            name: "Remote".into(),
            url,
            username: Some(USER.into()),
            password: Some(PASS.into()),
        })
        .await
        .expect("add remote");
    let mut state = test_server_state();
    state.remote = registry.clone();
    let home = start_mock_upstream(test_router(state)).await;
    Setup {
        fake,
        home,
        id: snapshot.id,
        registry,
    }
}
