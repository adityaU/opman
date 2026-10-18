use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::mcp_code::testing::pool_with;

#[test]
fn parse_requires_a_string_code() {
    assert!(parse(None).is_err());
    let wrong = json!({"code": 1});
    assert!(parse(Some(&wrong)).is_err());
    let args = json!({"code": "return 1"});
    let ok = parse(Some(&args)).expect("ok");
    assert_eq!(
        ok,
        Input {
            code: "return 1",
            timeout_ms: None
        }
    );
}

#[test]
fn parse_reads_the_timeout() {
    let with = |value: serde_json::Value| json!({"code": "x", "timeout_ms": value});
    let args = with(json!(5000));
    assert_eq!(parse(Some(&args)).expect("ok").timeout_ms, Some(5000));
    let args = with(json!("soon"));
    assert!(parse(Some(&args)).is_err());
    let args = with(json!(-1));
    assert!(parse(Some(&args)).is_err());
    let args = with(json!(null));
    assert_eq!(parse(Some(&args)).expect("ok").timeout_ms, None);
}

#[tokio::test]
async fn the_definition_carries_the_generated_description() {
    let pool = pool_with("fake");
    let catalog = pool.catalog().await;
    let def = definition(&catalog);
    assert_eq!(def["name"], "execute");
    assert_eq!(def["inputSchema"]["required"], json!(["code"]));
    let description = def["description"].as_str().unwrap_or_default();
    assert!(description.contains("await tools.fake.add("));
}

#[tokio::test]
async fn a_script_runs_against_a_real_connection() {
    let pool = Arc::new(pool_with("fake"));
    let code = "const [a, b] = await Promise.all([tools.fake.add({a: 1, b: 2}), tools.fake.add({a: 10, b: 20})]); return a + b";
    let result = call(
        &pool,
        Input {
            code,
            timeout_ms: None,
        },
        Cancel::default(),
    )
    .await;
    assert_eq!(result["content"][0]["text"], "33");
    let calls = result["structuredContent"]["calls"]
        .as_array()
        .expect("calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["tool"], "fake.add");
}

#[tokio::test]
async fn a_typo_gets_a_suggestion() {
    let pool = Arc::new(pool_with("fake"));
    let code = "return await tools.fake.ad({})";
    let result = call(
        &pool,
        Input {
            code,
            timeout_ms: None,
        },
        Cancel::default(),
    )
    .await;
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("Did you mean tools.fake.add"), "{text}");
    assert_eq!(result["isError"], true);
}
