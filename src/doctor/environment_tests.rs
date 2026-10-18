use super::*;
use crate::doctor::report::Status;

#[test]
fn picks_the_last_problem_lines_in_order() {
    let log = "a INFO ok\nb ERROR one\nc INFO ok\nd WARN two\ne ERROR three\n";
    assert_eq!(last_problems(log, 2), vec!["d WARN two", "e ERROR three"]);
    assert!(last_problems("x INFO fine\n", 5).is_empty());
}

#[test]
fn a_clean_log_passes_and_a_noisy_one_warns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("opman.log");
    std::fs::write(&path, "t INFO fine\n").expect("write");
    assert_eq!(recent_problems(&path).status, Status::Pass);
    std::fs::write(&path, "t ERROR boom\n").expect("write");
    let check = recent_problems(&path);
    assert_eq!(check.status, Status::Warn);
    assert_eq!(check.detail.as_deref(), Some("t ERROR boom"));
}

#[test]
fn a_missing_log_is_not_a_failure_to_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        recent_problems(&dir.path().join("none.log")).status,
        Status::Warn
    );
}

#[test]
fn writable_dir_creates_the_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(writable_dir(&dir.path().join("a/b")).is_ok());
}

#[test]
fn a_read_only_log_path_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(log_writable(dir.path()).status, Status::Fail);
}
