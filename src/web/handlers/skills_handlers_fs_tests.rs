//! Success-path coverage for the skills FS handlers.
//!
//! `create_skill`/`update_skill`/`delete_skill`/`upload_skills` write into the
//! directory returned by `crate::mcp_skills::get_skills_dir()`, which resolves
//! via `dirs::config_dir()` → honours `XDG_CONFIG_HOME` on Linux. We redirect
//! that env var to a unique temp dir (under a serializing mutex so parallel
//! tests never clobber each other) and exercise the real filesystem writes.

use super::*;

use crate::web::test_support::{test_router, test_server_state};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use std::sync::Mutex;

/// Serializes XDG_CONFIG_HOME mutation across the tests in this module.
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as XDG_LOCK;

/// RAII guard that points `XDG_CONFIG_HOME` at a fresh temp dir and restores
/// the previous value (and the lock) on drop.
struct XdgRedirect {
    _tmp: tempfile::TempDir,
    prev: Option<std::ffi::OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl XdgRedirect {
    fn new() -> Self {
        let guard = XDG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        XdgRedirect {
            _tmp: tmp,
            prev,
            _guard: guard,
        }
    }

    fn skills_dir(&self) -> std::path::PathBuf {
        crate::mcp_skills::get_skills_dir()
    }
}

impl Drop for XdgRedirect {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}

fn req(name: &str, desc: &str, content: &str) -> CreateSkillRequest {
    CreateSkillRequest {
        name: name.into(),
        description: desc.into(),
        content: content.into(),
    }
}

#[tokio::test]
async fn create_skill_writes_skill_md() {
    let xdg = XdgRedirect::new();
    let state = test_server_state();
    let axum::Json(v) = create_skill(
        State(state),
        axum::Json(req("mysk", "does things", "hello body")),
    )
    .await
    .unwrap();
    assert_eq!(v["status"], "created");

    let md = xdg.skills_dir().join("mysk").join("SKILL.md");
    let body = std::fs::read_to_string(&md).unwrap();
    assert!(body.contains("name: mysk"));
    assert!(body.contains("description: does things"));
    assert!(body.contains("hello body"));
}

#[tokio::test]
async fn update_skill_success_overwrites() {
    let xdg = XdgRedirect::new();
    let state = test_server_state();
    // Seed via create, then update.
    create_skill(
        State(state.clone()),
        axum::Json(req("upd", "old", "old-content")),
    )
    .await
    .unwrap();
    let axum::Json(v) = update_skill(
        State(state),
        Path("upd".into()),
        axum::Json(req("upd", "new-desc", "new-content")),
    )
    .await
    .unwrap();
    assert_eq!(v["status"], "updated");

    let md = xdg.skills_dir().join("upd").join("SKILL.md");
    let body = std::fs::read_to_string(&md).unwrap();
    assert!(body.contains("new-desc"));
    assert!(body.contains("new-content"));
    assert!(!body.contains("old-content"));
}

#[tokio::test]
async fn delete_skill_success_removes_dir() {
    let xdg = XdgRedirect::new();
    let state = test_server_state();
    create_skill(State(state.clone()), axum::Json(req("del", "d", "c")))
        .await
        .unwrap();
    let dir = xdg.skills_dir().join("del");
    assert!(dir.exists());

    let axum::Json(v) = delete_skill(State(state), Path("del".into()))
        .await
        .unwrap();
    assert_eq!(v["status"], "deleted");
    assert!(!dir.exists());
}

#[tokio::test]
async fn update_skill_missing_still_404() {
    let _xdg = XdgRedirect::new();
    let state = test_server_state();
    let res = update_skill(
        State(state),
        Path("ghost_skill".into()),
        axum::Json(req("ghost_skill", "d", "c")),
    )
    .await;
    assert_eq!(res.unwrap_err(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_skill_missing_still_404() {
    let _xdg = XdgRedirect::new();
    let state = test_server_state();
    let res = delete_skill(State(state), Path("ghost_skill".into())).await;
    assert_eq!(res.unwrap_err(), StatusCode::NOT_FOUND);
}

// ── upload_skills success (valid ZIP through the real router) ───────────────

fn make_zip() -> Vec<u8> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    let mut buf = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zw.add_directory("uploaded/", opts).unwrap();
        zw.start_file("uploaded/SKILL.md", opts).unwrap();
        zw.write_all(b"---\nname: uploaded\ndescription: up\n---\nzipbody")
            .unwrap();
        zw.finish().unwrap();
    }
    buf
}

fn multipart_body(field: &str, filename: &str, payload: &[u8]) -> Vec<u8> {
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"--BOUND\r\n");
    b.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\r\n"
        )
        .as_bytes(),
    );
    b.extend_from_slice(payload);
    b.extend_from_slice(b"\r\n--BOUND--\r\n");
    b
}

async fn send_upload(router: axum::Router, body: Vec<u8>) -> StatusCode {
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
async fn upload_skills_valid_zip_extracts() {
    let xdg = XdgRedirect::new();
    // Ensure the skills dir exists (upload_skills joins into it without creating root).
    std::fs::create_dir_all(xdg.skills_dir()).unwrap();

    let state = test_server_state();
    let router = test_router(state);
    let body = multipart_body("skills_zip", "s.zip", &make_zip());
    let st = send_upload(router, body).await;
    assert_eq!(st, StatusCode::OK);

    let md = xdg.skills_dir().join("uploaded").join("SKILL.md");
    let body = std::fs::read_to_string(&md).unwrap();
    assert!(body.contains("zipbody"));
}
