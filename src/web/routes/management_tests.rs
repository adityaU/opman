use super::*;
use axum::{http::StatusCode, routing::post, Router};

fn router() -> Router {
    Router::new()
        .route("/management", post(call))
        .with_state(crate::web::test_support::test_server_state())
}

fn request(token: Option<&str>, operation: &str, args: Value) -> anyhow::Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/management")
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("x-internal-token", token);
    }
    Ok(builder.body(Body::from(
        json!({"operation":operation,"args":args}).to_string(),
    ))?)
}

#[tokio::test]
async fn management_always_authenticates_catalog_reads_and_writes() -> anyhow::Result<()> {
    for operation in [
        "catalog",
        "settings_projects_list",
        "settings_appearance_set",
    ] {
        for token in [None, Some("wrong-token"), Some("")] {
            let response = router()
                .oneshot(request(token, operation, json!({}))?)
                .await?;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
    Ok(())
}

#[tokio::test]
async fn management_catalog_is_available_with_valid_capability() -> anyhow::Result<()> {
    let response = router()
        .oneshot(request(Some("test-internal-token"), "catalog", json!({}))?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1_000_000).await?;
    let tools: Value = serde_json::from_slice(&bytes)?;
    let tools = tools
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("expected tool list"))?;
    assert!(tools.len() >= 40);
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "settings_runner_limits_set"
            && tool["inputSchema"]["properties"]["body"]["properties"]["modelSets"].is_object()));
    Ok(())
}

#[tokio::test]
async fn management_rejects_unknown_routes_and_path_injection() -> anyhow::Result<()> {
    for (operation, args) in [
        ("http", json!({"path":"/auth/login"})),
        (
            "settings_mcp_enabled",
            json!({"id":"../../auth/login","body":{"enabled":true}}),
        ),
        ("settings_projects_list", json!({"path":"/providers"})),
    ] {
        let response = router()
            .oneshot(request(Some("test-internal-token"), operation, args)?)
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    Ok(())
}

#[tokio::test]
async fn management_reads_reuse_authenticated_settings_handlers() -> anyhow::Result<()> {
    let mut state = crate::web::test_support::test_server_state();
    state.username = "protected".into();
    let router = Router::new()
        .route("/management", post(call))
        .with_state(state);
    let response = router
        .oneshot(request(
            Some("test-internal-token"),
            "settings_projects_list",
            json!({}),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await?)?;
    assert!(value["projects"].is_array());
    assert!(value.get("runners").is_none());
    Ok(())
}

#[tokio::test]
async fn management_uses_existing_body_validation() -> anyhow::Result<()> {
    let response = router()
        .oneshot(request(
            Some("test-internal-token"),
            "settings_appearance_set",
            json!({"body":{"density":"huge"}}),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[test]
fn management_output_hides_credentials_but_preserves_tool_schemas() {
    let mut value = json!({"env":{"KEY":"secret-env"},"password":"secret-password","args":["--token","secret-token","--api-key=secret-key","--header=secret-header"],"url":"https://user:password@example.com/path?key=secret-key&view=1", "inputSchema":{"properties":{"token":{"type":"string"},"env":{"type":"object"}}}});
    output::redact(&mut value);
    let text = value.to_string();
    for secret in [
        "secret-env",
        "secret-password",
        "secret-token",
        "secret-key",
        "secret-header",
        "user:password",
    ] {
        assert!(!text.contains(secret), "leaked {secret}");
    }
    assert_eq!(value["url"], "https://example.com/path?view=1");
    assert_eq!(
        value["inputSchema"]["properties"]["token"]["type"],
        "string"
    );
    assert_eq!(output::section("settings_skill_create"), "skills");
    assert_eq!(output::section("settings_server_add"), "servers");
    assert_eq!(output::section("settings_project_add"), "projects");
}
