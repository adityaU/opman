use super::*;

#[test]
fn web_session_stats_default_and_skip_empty_id() {
    let d = WebSessionStats::default();
    assert_eq!(d.session_id, "");
    assert_eq!(d.cost, 0.0);
    // Empty session_id is skipped.
    let v = serde_json::to_value(&d).unwrap();
    assert!(v.get("session_id").is_none());
    assert_eq!(v["cost"], 0.0);
    assert_eq!(v["input_tokens"], 0);
    assert!(format!("{:?}", d.clone()).contains("WebSessionStats"));
}

#[test]
fn web_session_stats_full() {
    let s = WebSessionStats {
        session_id: "s1".into(),
        cost: 1.5,
        input_tokens: 100,
        output_tokens: 200,
        reasoning_tokens: 50,
        cache_read: 10,
        cache_write: 20,
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["cost"], 1.5);
    assert_eq!(v["reasoning_tokens"], 50);
    assert_eq!(v["cache_read"], 10);
    assert_eq!(v["cache_write"], 20);
}

#[test]
fn context_window_response_serialize() {
    let resp = ContextWindowResponse {
        context_limit: 200000,
        total_used: 1000,
        usage_pct: 0.5,
        categories: vec![ContextCategory {
            name: "system".into(),
            label: "System".into(),
            tokens: 500,
            pct: 50.0,
            color: "blue".into(),
            items: vec![ContextItem {
                label: "sys prompt".into(),
                tokens: 500,
            }],
        }],
        estimated_messages_remaining: Some(42),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["context_limit"], 200000);
    assert_eq!(v["categories"][0]["name"], "system");
    assert_eq!(v["categories"][0]["items"][0]["tokens"], 500);
    assert_eq!(v["estimated_messages_remaining"], 42);
    assert!(format!("{:?}", resp.clone()).contains("ContextWindowResponse"));
}

#[test]
fn context_window_response_none_remaining() {
    let resp = ContextWindowResponse {
        context_limit: 1,
        total_used: 0,
        usage_pct: 0.0,
        categories: vec![],
        estimated_messages_remaining: None,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v["estimated_messages_remaining"].is_null());
}

#[test]
fn agent_entry_with_and_without_color() {
    let with = AgentEntry {
        id: "coder".into(),
        label: "Coder".into(),
        description: "d".into(),
        mode: "primary".into(),
        hidden: false,
        native: true,
        color: Some("#fff".into()),
    };
    let v = serde_json::to_value(&with).unwrap();
    assert_eq!(v["id"], "coder");
    assert_eq!(v["mode"], "primary");
    assert_eq!(v["native"], true);
    assert_eq!(v["color"], "#fff");

    let without = AgentEntry {
        id: "task".into(),
        label: "Task".into(),
        description: "".into(),
        mode: "subagent".into(),
        hidden: true,
        native: false,
        color: None,
    };
    let v2 = serde_json::to_value(&without).unwrap();
    assert!(v2.get("color").is_none()); // skip_serializing_if None
    assert_eq!(v2["hidden"], true);
}
