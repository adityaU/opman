//! Authenticated management MCP. Settings use the same validation and storage as the UI.
pub(crate) mod catalog;
mod catalog_tools;
mod operation;
mod schemas;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

#[derive(Deserialize)]
struct Request {
    method: String,
    #[serde(default)]
    id: Value,
    #[serde(default)]
    params: Value,
}

pub async fn run_bridge() -> anyhow::Result<()> {
    bridge(tokio::io::stdin(), tokio::io::stdout()).await
}

async fn bridge<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: R,
    mut writer: W,
) -> anyhow::Result<()> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => route(request).await,
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}}),
            ),
        };
        let Some(response) = response else {
            continue;
        };
        writer
            .write_all(serde_json::to_string(&response)?.as_bytes())
            .await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }
}

async fn route(request: Request) -> Option<Value> {
    if request.id.is_null() {
        return None;
    }
    let result = match request.method.as_str() {
        "initialize" => Ok(
            json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"opman-management","version":"1.0.0"}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => authenticated_call("catalog", json!({}))
            .await
            .map(|tools| json!({"tools":tools})),
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let value = authenticated_call(name, args).await;
            let (content, failed) = match value {
                Ok(value) => (value.to_string(), false),
                Err(error) => (error, true),
            };
            Ok(json!({"content":[{"type":"text","text":content}],"isError":failed}))
        }
        _ => Err("Unknown MCP method".into()),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":request.id,"result":result}),
        Err(message) => {
            json!({"jsonrpc":"2.0","id":request.id,"error":{"code":-32001,"message":message}})
        }
    })
}

fn validate_descriptor(internal: &crate::loopback::Loopback) -> Result<(), String> {
    let url = reqwest::Url::parse(&internal.url).map_err(|_| "Invalid management endpoint")?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => name == "localhost",
        None => false,
    };
    if !loopback
        || url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
        || internal.token.is_empty()
    {
        return Err("Management requires an authenticated local opman endpoint".into());
    }
    Ok(())
}

async fn authenticated_call(operation: &str, args: Value) -> Result<Value, String> {
    // Reload per call: restarting opman rotates the shared capability token.
    let internal =
        crate::loopback::Loopback::load().ok_or("Management needs a running opman web server")?;
    validate_descriptor(&internal)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Cannot create management client")?;
    let response = client
        .post(format!(
            "{}/internal/management",
            internal.url.trim_end_matches('/')
        ))
        .header("x-internal-token", &internal.token)
        .json(&json!({"operation":operation,"args":args}))
        .send()
        .await
        .map_err(|_| "Cannot reach the authenticated management endpoint")?;
    if !response.status().is_success() {
        return Err(format!("Management request failed ({})", response.status()));
    }
    response
        .json()
        .await
        .map_err(|_| "Invalid management response".into())
}

#[cfg(test)]
mod tests;
