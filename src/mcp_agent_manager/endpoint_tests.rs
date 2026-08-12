use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::PathBuf;
use std::sync::Mutex;

use tokio::net::UnixStream;

use super::*;

fn socket_in(directory: &std::path::Path, name: &str) -> PathBuf {
    directory.join(format!("{name}-{}.sock", std::process::id()))
}

#[tokio::test]
async fn unlinking_the_path_is_healed_before_a_fresh_client_connects() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = socket_in(directory.path(), "unlink");
    let endpoint = Endpoint::bind(path.clone()).expect("bind endpoint");
    std::fs::remove_file(&path).expect("unlink socket path");

    let (endpoint, result) = endpoint.tick();
    result.expect("supervisor should rebind");
    UnixStream::connect(&path)
        .await
        .expect("fresh client should reach the rebound listener");
    drop(endpoint);
    assert!(!path.exists(), "shutdown should remove the socket");
}

#[tokio::test]
async fn replacing_the_file_with_another_socket_is_detected_by_inode() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = socket_in(directory.path(), "inode");
    let endpoint = Endpoint::bind(path.clone()).expect("bind endpoint");
    let original_inode = std::fs::metadata(&path).expect("original socket").ino();
    std::fs::remove_file(&path).expect("unlink original socket");
    let other = StdUnixListener::bind(&path).expect("bind replacement socket");
    let other_inode = std::fs::metadata(&path).expect("replacement socket").ino();

    let (endpoint, result) = endpoint.tick();
    result.expect("supervisor should replace the other socket");
    let rebound_inode = std::fs::metadata(&path).expect("rebound socket").ino();
    assert_ne!(
        other_inode, rebound_inode,
        "existence alone would miss this replacement"
    );
    assert_ne!(
        original_inode, rebound_inode,
        "the endpoint must bind a fresh inode"
    );

    drop(endpoint);
    drop(other);
}

#[tokio::test]
async fn dropping_the_endpoint_removes_its_socket_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = socket_in(directory.path(), "shutdown");
    let endpoint = Endpoint::bind(path.clone()).expect("bind endpoint");

    assert!(path.exists());
    drop(endpoint);
    assert!(
        !path.exists(),
        "graceful endpoint shutdown should unlink the file"
    );
}

#[test]
fn socket_path_uses_runtime_dir_and_falls_back_when_unset() {
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _lock = ENV_LOCK.lock().expect("environment lock");
    let original = std::env::var_os("XDG_RUNTIME_DIR");
    let directory = tempfile::tempdir().expect("temporary directory");
    std::env::set_var("XDG_RUNTIME_DIR", directory.path());
    assert_eq!(
        crate::mcp_agent_manager::socket_path().parent(),
        Some(directory.path())
    );

    std::env::remove_var("XDG_RUNTIME_DIR");
    let temp_directory = std::env::temp_dir();
    assert_eq!(
        crate::mcp_agent_manager::socket_path().parent(),
        Some(temp_directory.as_path())
    );

    match original {
        Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
        None => std::env::remove_var("XDG_RUNTIME_DIR"),
    }
}
