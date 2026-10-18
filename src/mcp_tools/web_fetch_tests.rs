use super::web_fetch::{run, FetchArgs};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn args(value: serde_json::Value) -> FetchArgs {
    serde_json::from_value(value).expect("test fixture or assertion")
}

/// A one-shot local server that answers every request with `response`.
async fn serve(response: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test fixture or assertion");
    let addr = listener.local_addr().expect("test fixture or assertion");
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let response = response.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}/")
}

fn http(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn html_becomes_text() {
    let url = serve(http(
        "200 OK",
        "text/html",
        "<h1>Hi</h1><p>there <a href=\"/x\">you</a></p>",
    ))
    .await;
    let out = run(args(json!({"url": url})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(out.data["status"], 200);
    assert_eq!(out.data["content"], "# Hi\n\nthere [you](/x)");
    assert_eq!(out.data["truncated"], false);
}

#[tokio::test]
async fn download_is_capped() {
    let url = serve(http("200 OK", "text/plain", &"a".repeat(5000))).await;
    let out = run(args(json!({"url": url, "max_bytes": 100})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(
        out.data["content"]
            .as_str()
            .expect("test fixture or assertion")
            .len(),
        100
    );
    assert_eq!(out.data["truncated"], true);
    assert!(out.text.contains("max_bytes"));
}

#[tokio::test]
async fn error_status_and_binary_type_are_errors() {
    let url = serve(http("404 Not Found", "text/plain", "gone")).await;
    let out = run(args(json!({"url": url})))
        .await
        .expect("test fixture or assertion");
    assert!(out.is_error);
    assert_eq!(out.data["status"], 404);
    let image = serve(http("200 OK", "image/png", "xx")).await;
    let err = run(args(json!({"url": image})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(err.message().contains("not text"));
}

#[tokio::test]
async fn only_http_and_https_are_allowed() {
    let file = run(args(json!({"url": "file:///etc/passwd"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(file.message().contains("only http and https"));
    let junk = run(args(json!({"url": "not a url"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(junk.message().contains("invalid url"));
    let down = run(args(json!({"url": "http://127.0.0.1:1/"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(down.message().contains("request failed"));
}
