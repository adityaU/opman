//! Coverage for the optional-argument branches of `spawn` (model, agent,
//! `--resume`, `--mcp-config`, engine-url env, and the empty-permission-mode →
//! `bypassPermissions` fallback), driven through `send` with a fake `claude`
//! binary. Env access is serialized under the shared `ENV_LOCK`.

use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use std::os::unix::fs::PermissionsExt;

/// Install a permissive fake `claude` that ignores its args, emits a clean
/// result, then lingers briefly so the child stays alive to be inspected.
fn write_fake_claude(dir: &std::path::Path) -> std::path::PathBuf {
    let script = dir.join("fake-claude");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\"}'\n\
         sleep 0.3\n",
    )
    .unwrap();
    let mut perm = std::fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&script, perm).unwrap();
    script
}

#[tokio::test]
async fn spawn_builds_all_optional_args() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();

    let bindir = tempfile::tempdir().unwrap();
    let script = write_fake_claude(bindir.path());
    std::env::set_var("OPMAN_CLAUDE_BIN", &script);

    // A real cwd is required (Command::current_dir).
    let cwd = tempfile::tempdir().unwrap();
    // All mcp flags on → `mcp_config` is non-empty → `--mcp-config` branch.
    let e = Arc::new(ClaudePEngine::new(None, (true, true, true, true)));
    e.set_url("http://127.0.0.1:9"); // engine_url non-empty → OPMAN_ENGINE_URL env branch
    let s = e.create_session(&cwd.path().to_string_lossy(), "", "A");
    e.set_model(&s.id, "my-model"); // model Some → `--model`
    e.set_agent(&s.id, "my-agent"); // agent Some non-empty → `--agent`
    e.set_claude_uuid(&s.id, "resume-uuid"); // resume Some → `--resume`
    e.set_permission_mode(&s.id, ""); // empty → `bypassPermissions` literal branch

    send(e.clone(), s.id.clone(), "hi".to_string()).await;
    // Spawn succeeded with every optional arg populated.
    assert!(
        e.procs.0.lock().await.contains_key(&s.id),
        "fake claude spawned"
    );
    assert!(e.get_session(&s.id).unwrap().busy);

    abort(e.clone(), &s.id).await;
    assert!(e.procs.0.lock().await.is_empty());

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}

#[tokio::test]
async fn spawn_minimal_args_default_mode() {
    // The complementary shape: no model/agent/resume/url and mcp disabled, with a
    // non-empty (default) permission mode — exercises the "skip optional args"
    // side of each branch and the `&opts.permission_mode` (non-empty) arm.
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();

    let bindir = tempfile::tempdir().unwrap();
    let script = write_fake_claude(bindir.path());
    std::env::set_var("OPMAN_CLAUDE_BIN", &script);

    let cwd = tempfile::tempdir().unwrap();
    let e = Arc::new(ClaudePEngine::new(None, (false, false, false, false)));
    let s = e.create_session(&cwd.path().to_string_lossy(), "", "A");

    send(e.clone(), s.id.clone(), "hi".to_string()).await;
    assert!(e.procs.0.lock().await.contains_key(&s.id));

    abort(e.clone(), &s.id).await;

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}
