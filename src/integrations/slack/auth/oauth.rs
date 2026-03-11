//! Slack OAuth 2.0 flow — browser-based authorization with local callback server.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::{info, warn};

use super::SlackAuth;

#[allow(dead_code)] // Used in run_oauth_flow.
const BOT_SCOPES: &str = "chat:write,im:history,im:read,im:write,users:read";

/// Fixed port for the OAuth callback server so the redirect URI is predictable.
/// Configure `http://127.0.0.1:17380/slack/oauth/callback` in your Slack app.
const OAUTH_CALLBACK_PORT: u16 = 17380;

/// Run the OAuth flow: open browser → local callback server → exchange code → save.
/// Returns the populated `SlackAuth` on success.
#[allow(dead_code)] // Wired via SlackOAuth command action.
pub async fn run_oauth_flow(client_id: &str, client_secret: &str) -> Result<SlackAuth> {
    // Bind the fixed callback port.
    let listener = TcpListener::bind(format!("127.0.0.1:{}", OAUTH_CALLBACK_PORT))
        .await
        .with_context(|| {
            format!(
                "Port {} already in use – is another OAuth flow running?",
                OAUTH_CALLBACK_PORT
            )
        })?;
    let redirect_uri = format!(
        "http://127.0.0.1:{}/slack/oauth/callback",
        OAUTH_CALLBACK_PORT
    );

    // Generate a random state parameter for CSRF protection.
    let state: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect()
    };

    let auth_url = format!(
        "https://slack.com/oauth/v2/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
        client_id,
        BOT_SCOPES,
        urlencoding(&redirect_uri),
        state,
    );

    info!("Opening Slack OAuth URL in browser...");
    if let Err(e) = open::that(&auth_url) {
        warn!(
            "Could not open browser automatically: {}. Please visit: {}",
            e, auth_url
        );
    }

    // Wait for the callback.
    let code = wait_for_oauth_callback(listener, &state).await?;

    // Exchange the code for tokens.
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/oauth.v2.access")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ])
        .send()
        .await
        .context("Failed to exchange OAuth code")?;

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse OAuth response")?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("Slack OAuth failed: {}", err);
    }

    let bot_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .context("Missing access_token in OAuth response")?
        .to_string();

    // Get the authenticated user's ID.
    let user_id = fetch_auth_user_id(&client, &bot_token).await?;

    // Prompt user for app-level token (Socket Mode requires manual creation in Slack UI).
    // For now, we'll need the user to provide this. Store empty and let them fill it in.
    info!("OAuth complete. Bot token obtained. User ID: {}", user_id);
    info!("NOTE: You must create an App-Level Token in your Slack app settings");
    info!("      (Settings → Basic Information → App-Level Tokens)");
    info!(
        "      with the `connections:write` scope, then add it to slack_auth.yaml as `app_token`."
    );

    let auth = SlackAuth {
        bot_token,
        app_token: String::new(), // User must fill this in manually
        user_id,
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    auth.save()?;

    Ok(auth)
}

/// Wait for the OAuth callback on the local TCP listener.
/// Parses the `code` and `state` query parameters from the GET request.
#[allow(dead_code)] // Called from run_oauth_flow.
async fn wait_for_oauth_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    info!(
        "Waiting for OAuth callback on {}...",
        listener.local_addr()?
    );

    let (mut stream, _addr) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .context("OAuth callback timed out (5 minutes)")?
        .context("Failed to accept OAuth callback connection")?;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the first line: GET /slack/oauth/callback?code=...&state=... HTTP/1.1
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    let query = if let Some(q) = path.split('?').nth(1) {
        q
    } else {
        // Send error response.
        let resp = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<h1>Missing query parameters</h1>";
        stream.write_all(resp.as_bytes()).await.ok();
        anyhow::bail!("OAuth callback missing query parameters");
    };

    let params: HashMap<&str, &str> = query
        .split('&')
        .filter_map(|p| {
            let mut parts = p.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();

    let code = params.get("code").context("Missing 'code' parameter")?;
    let recv_state = params.get("state").context("Missing 'state' parameter")?;

    if *recv_state != expected_state {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\n\r\n<h1>State mismatch — possible CSRF</h1>";
        stream.write_all(resp.as_bytes()).await.ok();
        anyhow::bail!("OAuth state mismatch");
    }

    // Send success response.
    let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body style='font-family:sans-serif;text-align:center;padding:40px'>\
        <h1>&#10004; Slack Connected!</h1>\
        <p>You can close this tab and return to opman.</p>\
        </body></html>";
    stream.write_all(resp.as_bytes()).await.ok();

    Ok(code.to_string())
}

/// Fetch the authenticated user's Slack ID using auth.test.
#[allow(dead_code)] // Called from run_oauth_flow.
async fn fetch_auth_user_id(client: &reqwest::Client, bot_token: &str) -> Result<String> {
    let resp = client
        .post("https://slack.com/api/auth.test")
        .bearer_auth(bot_token)
        .send()
        .await
        .context("Failed to call auth.test")?;

    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("auth.test failed: {}", err);
    }

    body.get("user_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing user_id in auth.test response")
}

/// Minimal percent-encoding for URL query parameters.
#[allow(dead_code)] // Called from run_oauth_flow.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}
