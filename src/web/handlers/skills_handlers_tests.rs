//! Generated coverage tests for `skills_handlers.rs`.
//!
//! NOTE: `create_skill`/`update_skill`/`delete_skill`/`upload_skills` success
//! paths write into the real `~/.config/opman/skills` directory (there is no
//! override hook), so per the test guide we do NOT exercise those — only the
//! validation / not-found / bad-input branches and the registry-read handlers.

use super::*;

use crate::mcp_skills::Skill;
use crate::web::test_support::{test_router, test_server_state};
use axum::extract::{Path, State};
use axum::http::StatusCode;

fn unique_absent_name() -> String {
    format!("opman_gen_test_absent_{}", rand::random::<u64>())
}

// ── list_skills ────────────────────────────────────────────────────

#[tokio::test]
async fn list_skills_empty() {
    let state = test_server_state();
    let axum::Json(v) = list_skills(State(state)).await.unwrap();
    assert_eq!(v.len(), 0);
}

#[tokio::test]
async fn list_skills_with_seeded() {
    let state = test_server_state();
    {
        let mut reg = state.skills_registry.write().await;
        reg.insert(
            "demo".into(),
            Skill {
                name: "demo".into(),
                description: "d".into(),
                content: "c".into(),
            },
        );
    }
    let axum::Json(v) = list_skills(State(state)).await.unwrap();
    assert_eq!(v.len(), 1);
}

// ── get_skill ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_skill_present_and_absent() {
    let state = test_server_state();
    {
        let mut reg = state.skills_registry.write().await;
        reg.insert(
            "demo".into(),
            Skill {
                name: "demo".into(),
                description: "d".into(),
                content: "c".into(),
            },
        );
    }
    let axum::Json(found) = get_skill(State(state.clone()), Path("demo".into()))
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "demo");

    let axum::Json(missing) = get_skill(State(state), Path("nope".into())).await.unwrap();
    assert!(missing.is_none());
}

// ── create_skill validation ────────────────────────────────────────

#[tokio::test]
async fn create_skill_empty_name_400() {
    let state = test_server_state();
    let res = create_skill(
        State(state),
        axum::Json(CreateSkillRequest {
            name: String::new(),
            description: "d".into(),
            content: "c".into(),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_skill_empty_description_400() {
    let state = test_server_state();
    let res = create_skill(
        State(state),
        axum::Json(CreateSkillRequest {
            name: "x".into(),
            description: String::new(),
            content: "c".into(),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err(), StatusCode::BAD_REQUEST);
}

// ── update_skill / delete_skill not-found ──────────────────────────

#[tokio::test]
async fn update_skill_missing_404() {
    let state = test_server_state();
    let res = update_skill(
        State(state),
        Path(unique_absent_name()),
        axum::Json(CreateSkillRequest {
            name: "x".into(),
            description: "d".into(),
            content: "c".into(),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_skill_missing_404() {
    let state = test_server_state();
    let res = delete_skill(State(state), Path(unique_absent_name())).await;
    assert_eq!(res.unwrap_err(), StatusCode::NOT_FOUND);
}

// ── upload_skills (multipart, error paths only) ────────────────────

async fn send_multipart(router: axum::Router, body: Vec<u8>) -> StatusCode {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("POST")
        .uri("/api/skills/upload")
        .header("content-type", "multipart/form-data; boundary=BOUND")
        .body(Body::from(body))
        .unwrap();
    router.oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn upload_skills_wrong_field_400() {
    let state = test_server_state();
    let router = test_router(state);
    let mut b = String::new();
    b.push_str("--BOUND\r\n");
    b.push_str("Content-Disposition: form-data; name=\"other\"\r\n\r\n");
    b.push_str("junk");
    b.push_str("\r\n--BOUND--\r\n");
    assert_eq!(
        send_multipart(router, b.into_bytes()).await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn upload_skills_invalid_zip_400() {
    let state = test_server_state();
    let router = test_router(state);
    let mut b = String::new();
    b.push_str("--BOUND\r\n");
    b.push_str("Content-Disposition: form-data; name=\"skills_zip\"; filename=\"s.zip\"\r\n\r\n");
    b.push_str("this-is-not-a-valid-zip-archive");
    b.push_str("\r\n--BOUND--\r\n");
    assert_eq!(
        send_multipart(router, b.into_bytes()).await,
        StatusCode::BAD_REQUEST
    );
}
