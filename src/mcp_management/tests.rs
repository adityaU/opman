use super::*;

#[test]
fn management_catalog_covers_every_settings_section() {
    let names: Vec<_> = catalog::operations()
        .into_iter()
        .map(|operation| operation.name)
        .collect();
    for expected in [
        "settings_appearance_set",
        "settings_keybindings_set",
        "settings_server_add",
        "settings_project_add",
        "settings_runner_enabled",
        "settings_agent_native_tools",
        "settings_runner_limits_set",
        "settings_mcp_upsert",
        "settings_skill_create",
    ] {
        assert!(names.contains(&expected), "missing {expected}");
    }
    let unique: std::collections::BTreeSet<_> = names.iter().collect();
    assert_eq!(names.len(), unique.len());
}

#[test]
fn management_targets_cannot_escape_their_settings_route() -> anyhow::Result<()> {
    let operations = catalog::operations();
    let operation = operations
        .iter()
        .find(|operation| operation.name == "settings_mcp_enabled")
        .ok_or_else(|| anyhow::anyhow!("missing tool"))?;
    for id in [
        "..",
        ".",
        "../auth/login",
        "%2f",
        "a?x",
        "a#b",
        "a\\b",
        "a\nb",
        "",
    ] {
        assert!(
            operation.target(&json!({"id":id})).is_err(),
            "accepted {id:?}"
        );
    }
    assert_eq!(
        operation
            .target(&json!({"id":"opman-tools"}))
            .map_err(anyhow::Error::msg)?,
        "/mcp/servers/opman-tools/enabled"
    );
    let models = operations
        .iter()
        .find(|operation| operation.name == "settings_runner_models")
        .ok_or_else(|| anyhow::anyhow!("missing model discovery"))?;
    assert_eq!(
        models
            .target(&json!({"id":"codex"}))
            .map_err(anyhow::Error::msg)?,
        "/providers?runner=codex"
    );
    assert_eq!(
        models.definition()["inputSchema"]["required"],
        json!(["id"])
    );
    Ok(())
}

#[test]
fn management_rejects_nonlocal_or_unauthenticated_descriptors() {
    for url in [
        "http://127.0.0.1.evil.test:1234",
        "https://example.com",
        "http://127.0.0.1:1234/api",
        "http://name:pass@localhost:1234",
        "http://localhost:1234?token=secret",
    ] {
        let descriptor = crate::loopback::Loopback {
            url: url.into(),
            token: "secret".into(),
            client: reqwest::Client::new(),
        };
        assert!(validate_descriptor(&descriptor).is_err(), "accepted {url}");
    }
    for url in [
        "http://127.0.0.1:1234",
        "http://[::1]:1234",
        "http://localhost:1234",
    ] {
        let descriptor = crate::loopback::Loopback {
            url: url.into(),
            token: "secret".into(),
            client: reqwest::Client::new(),
        };
        assert!(validate_descriptor(&descriptor).is_ok());
    }
}

#[tokio::test]
async fn management_protocol_handles_initialization_and_notifications() {
    assert!(route(Request {
        method: "notifications/initialized".into(),
        id: Value::Null,
        params: Value::Null
    })
    .await
    .is_none());
    let response = route(Request {
        method: "initialize".into(),
        id: json!("init-1"),
        params: Value::Null,
    })
    .await;
    assert_eq!(
        response
            .as_ref()
            .map(|value| &value["result"]["serverInfo"]["name"]),
        Some(&json!("opman-management"))
    );
}
