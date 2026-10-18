use super::*;
use std::process::Stdio;
use tokio::process::Command;

fn shell(script: &str) -> Child {
    Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sh")
}

#[test]
fn compose_keeps_reason_exit_and_stderr() {
    let text = compose("ACP `initialize` failed: boom", None, "trace line");
    assert_eq!(
        text,
        "ACP `initialize` failed: boom\nAgent stderr (last part):\ntrace line"
    );
}

#[test]
fn compose_with_nothing_extra_is_the_reason() {
    assert_eq!(compose("just this", None, ""), "just this");
}

#[test]
fn push_tail_keeps_only_the_end() {
    let bytes = Mutex::new(Vec::new());
    push_tail(&bytes, &vec![b'a'; KEEP_BYTES]);
    push_tail(&bytes, b"zz");
    let kept = bytes.lock().expect("lock");
    assert_eq!(kept.len(), KEEP_BYTES);
    assert!(kept.ends_with(b"zz"));
}

#[tokio::test]
async fn a_dying_agent_reports_status_and_stderr() {
    let mut child = shell("echo 'missing token' >&2; exit 3");
    let tail = StderrTail::capture(&mut child);
    let error = startup_failure(&mut child, &tail, anyhow!("acp process exited")).await;
    let text = format!("{error}");
    assert!(text.contains("acp process exited"), "{text}");
    assert!(text.contains("exit status: 3"), "{text}");
    assert!(text.contains("missing token"), "{text}");
}

#[tokio::test]
async fn a_child_without_a_pipe_has_an_empty_tail() {
    let mut child = Command::new("sh")
        .args(["-c", "exit 0"])
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sh");
    let tail = StderrTail::capture(&mut child);
    assert_eq!(tail.text().await, "");
}
