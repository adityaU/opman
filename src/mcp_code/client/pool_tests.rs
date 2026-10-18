use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::mcp_code::testing::{pool_with, spawn_fake};

#[tokio::test]
async fn catalog_lists_the_server_under_its_namespace() {
    let pool = pool_with("my-server");
    let catalog = pool.catalog().await;
    assert!(catalog.unavailable.is_empty());
    let group = catalog.group("my_server").expect("namespace");
    assert_eq!(group.tools.len(), 6);
}

#[tokio::test]
async fn nothing_starts_until_it_is_needed() {
    let opened = Arc::new(AtomicUsize::new(0));
    let counter = opened.clone();
    let open: Opener = Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            let (r, w, _) = spawn_fake();
            Connection::open(r, w, None)
                .await
                .map_err(|e| e.to_string())
        })
    });
    let ns = crate::mcp_code::names::Namespace::for_server("x", &Default::default());
    let pool = Pool::new(vec![Entry::new(ns, "x", open)]);
    assert_eq!(opened.load(Ordering::SeqCst), 0);
    pool.catalog().await;
    pool.catalog().await;
    pool.call("x", "add", json!({"a": 1, "b": 2}))
        .await
        .expect("call");
    assert_eq!(
        opened.load(Ordering::SeqCst),
        1,
        "one launch serves every use"
    );
}

#[tokio::test]
async fn calls_return_parsed_values() {
    let pool = pool_with("s");
    let out = pool
        .call("s", "add", json!({"a": 2, "b": 3}))
        .await
        .expect("call");
    assert_eq!(out.value, json!(5));
    let out = pool.call("s", "structured", json!({})).await.expect("call");
    assert_eq!(out.value, json!({"n": 7}));
}

#[tokio::test]
async fn tool_errors_carry_the_tool_text() {
    let pool = pool_with("s");
    let err = pool.call("s", "fail", json!({})).await.expect_err("error");
    assert_eq!(err, "boom");
}

#[tokio::test]
async fn a_server_that_will_not_open_is_reported_and_not_retried() {
    let opened = Arc::new(AtomicUsize::new(0));
    let counter = opened.clone();
    let open: Opener = Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err("could not start `nothing`".to_string()) })
    });
    let ns = crate::mcp_code::names::Namespace::for_server("bad", &Default::default());
    let pool = Pool::new(vec![Entry::new(ns, "bad", open)]);
    for _ in 0..3 {
        let catalog = pool.catalog().await;
        assert!(catalog.groups.is_empty());
        assert_eq!(catalog.unavailable[0].server.as_ref(), "bad");
        assert!(catalog.unavailable[0].reason.contains("could not start"));
    }
    assert_eq!(opened.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unknown_namespace_is_an_error() {
    let pool = pool_with("s");
    let err = pool.call("zzz", "add", json!({})).await.expect_err("error");
    assert!(err.contains("zzz"));
}
