//! Authenticated WebSocket connections from home to a remote.

use std::time::Duration;

use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::session::Remote;
use super::types::{Credentials, RemoteError};
use crate::mcp_oauth::Secret;

/// An open client WebSocket to a remote server.
pub type RemoteWs = WebSocketStream<MaybeTlsStream<TcpStream>>;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Open `{remote}{path}` (`path` like `/api/editor/ws?x=1`) with the remote's bearer,
/// logging in again once if the handshake is refused with a 401.
pub async fn connect(
    remote: &Remote,
    http: &reqwest::Client,
    path: &str,
) -> Result<RemoteWs, RemoteError> {
    let url = remote.record.url.ws(path);
    let bearer = remote.bearer(http).await?;
    let first = open(&url, bearer.as_ref()).await;
    let refused = matches!(&first, Err(Attempt::Unauthorized));
    if !refused || matches!(remote.record.credentials, Credentials::Open) {
        return first.map_err(Attempt::into_error);
    }
    let fresh = remote.relogin(http, bearer.as_ref()).await?;
    open(&url, fresh.as_ref()).await.map_err(Attempt::into_error)
}

enum Attempt {
    Unauthorized,
    Failed(String),
}

impl Attempt {
    fn into_error(self) -> RemoteError {
        match self {
            Self::Unauthorized => RemoteError::AuthFailed,
            Self::Failed(why) => RemoteError::WebSocket(why),
        }
    }
}

async fn open(url: &str, bearer: Option<&Secret>) -> Result<RemoteWs, Attempt> {
    let failed = |e: &dyn std::fmt::Display| Attempt::Failed(e.to_string());
    let mut request = url.into_client_request().map_err(|e| failed(&e))?;
    if let Some(token) = bearer {
        let value = HeaderValue::from_str(&format!("Bearer {}", token.expose()))
            .map_err(|e| failed(&e))?;
        request.headers_mut().insert(header::AUTHORIZATION, value);
    }
    let handshake = tokio_tungstenite::connect_async(request);
    match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await {
        Err(_) => Err(Attempt::Failed("handshake timed out".to_owned())),
        Ok(Ok((stream, _))) => Ok(stream),
        Ok(Err(WsError::Http(response))) if response.status() == StatusCode::UNAUTHORIZED => {
            Err(Attempt::Unauthorized)
        }
        Ok(Err(e)) => Err(failed(&e)),
    }
}
