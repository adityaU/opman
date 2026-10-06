use serde_json::{json, Value};

use super::fake_remote::{self, setup, Fake, PASS, USER};
use crate::remote::{Registry, ServerStore};
use crate::web::test_support::{
    start_mock_upstream, test_router, test_server_state, test_server_state_with_auth,
};

async fn call(method: reqwest::Method, url: String, body: Option<Value>) -> (u16, Value) {
    let mut request = reqwest::Client::new().request(method, url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

#[tokio::test]
async fn crud_round_trip_never_returns_passwords() {
    let s = setup().await;
    let mut changes = s.registry.subscribe();
    let remote_url = fake_remote::spawn(Fake::default()).await;
    let servers = format!("{}/api/servers", s.home);

    let wrong = json!({ "name": "Box", "url": remote_url, "username": USER, "password": "no" });
    let (status, body) = call(reqwest::Method::POST, servers.clone(), Some(wrong)).await;
    assert_eq!(status, 400);
    assert!(body["error"].as_str().unwrap().contains("rejected"), "{body}");
    assert!(!changes.has_changed().unwrap());

    let good = json!({ "name": "Box", "url": remote_url, "username": USER, "password": PASS });
    let (status, added) = call(reqwest::Method::POST, servers.clone(), Some(good)).await;
    assert_eq!(status, 200, "{added}");
    assert_eq!(added["id"], "box");
    assert_eq!(added["base"], "/remote/box");
    assert_eq!(added["status"], "ok");
    assert!(changes.has_changed().unwrap());
    changes.mark_unchanged();

    let (status, list) = call(reqwest::Method::GET, servers.clone(), None).await;
    assert_eq!(status, 200);
    let list = list.as_array().unwrap();
    assert_eq!(list[0], json!({ "id": "home", "name": "Home", "url": null, "base": "", "status": "ok" }));
    assert_eq!(list.len(), 3);
    assert!(list.iter().all(|s| s.get("password").is_none() && s.get("username").is_none()));

    let item = format!("{servers}/box");
    let (status, renamed) =
        call(reqwest::Method::PATCH, item.clone(), Some(json!({ "name": "Big Box" }))).await;
    assert_eq!(status, 200);
    assert_eq!((renamed["id"].as_str(), renamed["name"].as_str()), (Some("box"), Some("Big Box")));
    assert_eq!(renamed["status"], "ok", "absent password must keep the stored one");

    let (status, _) =
        call(reqwest::Method::PATCH, item.clone(), Some(json!({ "password": "bad" }))).await;
    assert_eq!(status, 400);

    let (status, _) = call(reqwest::Method::DELETE, item.clone(), None).await;
    assert_eq!(status, 204);
    let (status, _) = call(reqwest::Method::DELETE, item, None).await;
    assert_eq!(status, 404);
    assert_eq!(s.registry.servers().await.len(), 1);
}

#[tokio::test]
async fn duplicate_names_get_distinct_ids() {
    let s = setup().await;
    let url = fake_remote::spawn(Fake::default()).await;
    let body = json!({ "name": "Remote", "url": url, "username": USER, "password": PASS });
    let (_, added) = call(reqwest::Method::POST, format!("{}/api/servers", s.home), Some(body)).await;
    assert_eq!(added["id"], "remote-2");
}

#[tokio::test]
async fn dead_remote_lists_unreachable_and_proxies_502() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let entry = json!({ "servers": [{ "id": "dead", "name": "Dead", "url": "http://127.0.0.1:9" }] });
    std::fs::write(&path, entry.to_string()).unwrap();
    let mut state = test_server_state();
    state.remote = Registry::new(ServerStore::at(path));
    let home = start_mock_upstream(test_router(state)).await;

    let (_, list) = call(reqwest::Method::GET, format!("{home}/api/servers"), None).await;
    assert_eq!(list[1]["status"], "unreachable");
    let response = reqwest::get(format!("{home}/remote/dead/api/sessions")).await.unwrap();
    assert_eq!(response.status(), 502);
}

#[tokio::test]
async fn spa_routes_serve_index_and_404_unknown() {
    let s = setup().await;
    let no_redirect = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let get = |path: String| no_redirect.get(format!("{}{path}", s.home)).send();

    for path in [format!("/remote/{}/", s.id), format!("/remote/{}/settings/x", s.id)] {
        let response = get(path.clone()).await.unwrap();
        assert_eq!(response.status(), 200, "{path}");
        assert!(response.headers()["content-type"].to_str().unwrap().starts_with("text/html"));
    }
    let redirect = get(format!("/remote/{}", s.id)).await.unwrap();
    assert_eq!(redirect.status(), 307);
    assert_eq!(redirect.headers()["location"], format!("/remote/{}/", s.id).as_str());
    for path in ["/remote/ghost/".to_owned(), format!("/remote/{}/api", s.id)] {
        assert_eq!(get(path.clone()).await.unwrap().status(), 404, "{path}");
    }
}

#[tokio::test]
async fn api_and_proxy_require_home_auth_but_spa_does_not() {
    let home = start_mock_upstream(test_router(test_server_state_with_auth("me", "pw"))).await;
    let client = reqwest::Client::new();
    for path in ["/api/servers", "/remote/any/api/sessions"] {
        let response = client.get(format!("{home}{path}")).send().await.unwrap();
        assert_eq!(response.status(), 401, "{path}");
    }
    let page = client.get(format!("{home}/remote/any/")).send().await.unwrap();
    assert_eq!(page.status(), 200, "signed-out users must reach the login page");
}
