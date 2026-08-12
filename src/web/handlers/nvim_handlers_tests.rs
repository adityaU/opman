use super::*;

use crate::web::test_support::{send_json, test_router, test_server_state};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn browser_eval_is_forbidden() {
    let status = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/nvim",
        Some(json!({"op": "nvim_eval", "command": "return 1", "session_id": "s"})),
    )
    .await
    .0;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn browser_command_w_is_allowed() {
    let state = test_server_state();
    state.nvim_registry.write().await.insert(
        (0, "s".into()),
        std::env::temp_dir().join("missing-nvim.sock"),
    );
    let status = send_json(
        test_router(state),
        "POST",
        "/api/nvim",
        Some(json!({"op": "nvim_command", "command": "w", "session_id": "s"})),
    )
    .await
    .0;
    assert_ne!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn browser_command_rejects_shell_and_script_forms() {
    for command in [
        "!rm -rf /",
        "!sh",
        "source /tmp/x.lua",
        "lua vim.fn.system('id')",
    ] {
        let status = send_json(
            test_router(test_server_state()),
            "POST",
            "/api/nvim",
            Some(json!({"op": "nvim_command", "command": command, "session_id": "s"})),
        )
        .await
        .0;
        assert_eq!(status, StatusCode::FORBIDDEN, "command {command:?}");
    }
}

#[tokio::test]
async fn browser_nvim_requires_session_id() {
    let status = send_json(
        test_router(test_server_state()),
        "POST",
        "/api/nvim",
        Some(json!({"op": "nvim_read"})),
    )
    .await
    .0;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
