use super::*;

#[cfg(unix)]
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join(name);
    std::fs::write(&file, body).expect("script");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    file
}

fn not_found() -> io::Error {
    io::Error::from(io::ErrorKind::NotFound)
}

#[test]
fn a_missing_project_folder_is_named() {
    let fault = diagnose("npx", Path::new("/definitely/not/here"), &not_found());
    assert_eq!(
        fault,
        LaunchFault::MissingDir(PathBuf::from("/definitely/not/here"))
    );
}

#[test]
fn an_unknown_command_is_reported_as_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fault = diagnose("opman-no-such-agent-xyz", dir.path(), &not_found());
    assert_eq!(fault, LaunchFault::MissingCommand);
}

#[cfg(unix)]
#[test]
fn a_script_whose_interpreter_is_missing_names_the_interpreter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let npx = script(dir.path(), "npx", "#!/usr/bin/env node\nconsole.log(1)\n");
    let fault = diagnose(&npx.to_string_lossy(), dir.path(), &not_found());
    assert_eq!(fault, LaunchFault::MissingInterpreter("node".to_string()));
}

#[test]
fn interpreters_are_read_from_shebangs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cases = [
        ("#!/usr/bin/env node\n", Some("node")),
        ("#!/usr/bin/env -S node --no-warnings\n", Some("node")),
        ("#!/bin/sh\n", Some("sh")),
        ("\x7fELF binary", None),
    ];
    for (index, (head, expected)) in cases.into_iter().enumerate() {
        let file = dir.path().join(format!("f{index}"));
        std::fs::write(&file, head).expect("file");
        assert_eq!(interpreter(&file).as_deref(), expected, "{head:?}");
    }
}

#[test]
fn the_command_dir_goes_first_on_the_path_once() {
    let path = std::env::join_paths(["/usr/bin", "/Users/a/.n/bin", "/bin"]).expect("path");
    let merged = path_with_command_dir("/Users/a/.n/bin/npx", Some(path)).expect("merged");
    let dirs: Vec<PathBuf> = std::env::split_paths(&merged).collect();
    let expected: Vec<PathBuf> = ["/Users/a/.n/bin", "/usr/bin", "/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    assert_eq!(dirs, expected);
}

#[test]
fn a_bare_command_leaves_the_path_alone() {
    assert_eq!(
        path_with_command_dir("npx", Some(OsString::from("/usr/bin"))),
        None
    );
}

/// The reported bug: `npx` from a version manager's dir, with `node` only in that same
/// dir and not on opman's PATH. The command's dir in front of the PATH fixes the launch.
#[cfg(unix)]
#[tokio::test]
async fn a_script_finds_its_sibling_interpreter() {
    let bin = tempfile::tempdir().expect("tempdir");
    script(bin.path(), "fakenode", "#!/bin/sh\necho ran-by-fakenode\n");
    let agent_script = script(bin.path(), "agent", "#!/usr/bin/env fakenode\n");
    let agent = AgentConfig {
        command: agent_script.to_string_lossy().into_owned(),
        ..AgentConfig::default()
    };
    let mut child = spawn(&agent, &bin.path().to_string_lossy()).expect("spawned");
    let status = child.wait().await.expect("exited");
    assert!(status.success(), "{status}");
}

#[cfg(unix)]
#[test]
fn a_missing_folder_fails_with_its_name() {
    let agent = AgentConfig {
        command: "/bin/sh".to_string(),
        ..AgentConfig::default()
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let error = runtime
        .block_on(async { spawn(&agent, "/definitely/not/here").map(drop) })
        .expect_err("no such folder");
    assert!(
        error
            .to_string()
            .contains("project folder /definitely/not/here"),
        "{error}"
    );
}
