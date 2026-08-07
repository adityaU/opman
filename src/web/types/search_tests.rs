use super::*;
use serde_json::Value;

#[test]
fn search_result_entry_serializes() {
    let e = SearchResultEntry {
        session_id: "s".into(),
        session_title: "Title".into(),
        project_name: "proj".into(),
        message_id: "m1".into(),
        role: "user".into(),
        snippet: "hello world".into(),
        timestamp: 1_700_000_000,
    };
    let v: Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["session_id"], "s");
    assert_eq!(v["session_title"], "Title");
    assert_eq!(v["project_name"], "proj");
    assert_eq!(v["message_id"], "m1");
    assert_eq!(v["role"], "user");
    assert_eq!(v["snippet"], "hello world");
    assert_eq!(v["timestamp"], 1_700_000_000u64);
    let _ = format!("{e:?}");
    let _ = e.clone();
}

#[test]
fn search_response_serializes() {
    let resp = SearchResponse {
        query: "foo".into(),
        results: vec![SearchResultEntry {
            session_id: "s".into(),
            session_title: "t".into(),
            project_name: "p".into(),
            message_id: "m".into(),
            role: "assistant".into(),
            snippet: "match".into(),
            timestamp: 1,
        }],
        total: 1,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["query"], "foo");
    assert_eq!(v["total"], 1);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}
