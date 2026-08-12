use super::*;

use crate::nvim_ui::stream::wire::ControlMsg;
use crate::web::test_support::{test_router, test_server_state, test_server_state_with_auth};
use tokio::net::TcpListener;

#[tokio::test]
async fn unauthenticated_upgrade_is_rejected_before_session_creation() {
    let state = test_server_state_with_auth("user", "password");
    let headers = HeaderMap::new();
    assert!(matches!(
        authorize(&state, &headers, &None),
        Err(WebError::Unauthorized)
    ));
}

#[tokio::test]
async fn unauthenticated_websocket_upgrade_returns_401() {
    let router = test_router(test_server_state_with_auth("user", "password"));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("test listener address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("test server should stop without an error");
    });

    let result = tokio_tungstenite::connect_async(format!(
        "ws://{address}/api/nvim/ui?session_id=unauthorized"
    ))
    .await;
    server.abort();
    let _ = server.await;

    match result {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        }
        Ok(_) => panic!("unauthenticated handshake unexpectedly upgraded"),
        Err(error) => panic!("unexpected WebSocket handshake error: {error}"),
    }
}

#[tokio::test]
async fn auth_is_not_required_when_server_auth_is_disabled() {
    let state = test_server_state();
    assert!(authorize(&state, &HeaderMap::new(), &None).is_ok());
}

#[tokio::test]
async fn second_connection_supersedes_first_for_the_same_session() {
    let key = SessionKey::new(901, "supersede-test");
    let first = claim(key.clone());
    let second = claim(key.clone());
    let mut first = first;
    assert_eq!(first.receiver.recv().await, Some(ControlMsg::Superseded {}));
    release(&second);
    release(&first);
}

#[test]
fn edit_engine_lifecycle_keeps_existing_control_messages() {
    assert_eq!(
        serde_json::to_string(&ControlMsg::Ready {}).ok().as_deref(),
        Some(r#"{"type":"ready"}"#)
    );
}
