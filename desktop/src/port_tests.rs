//! The app reopens on the port it used last, so the page keeps its saved layout.

use std::net::{Ipv4Addr, TcpListener};

use super::*;

fn file() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("state").join("port");
    (dir, path)
}

#[test]
fn a_first_launch_picks_a_port_and_remembers_it() {
    let (_dir, path) = file();
    let port = Port::remembered(&path).expect("port");
    assert_eq!(std::fs::read_to_string(&path).expect("saved"), port.number().to_string());
}

#[test]
fn the_next_launch_reuses_the_remembered_port() {
    let (_dir, path) = file();
    let first = Port::remembered(&path).expect("port");
    let second = Port::remembered(&path).expect("port");
    assert_eq!(first, second);
}

#[test]
fn a_port_something_else_took_is_replaced() {
    let (_dir, path) = file();
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
    let busy = taken.local_addr().expect("addr").port();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(&path, busy.to_string()).expect("write");
    let port = Port::remembered(&path).expect("port");
    assert_ne!(port.number(), busy);
    assert_eq!(std::fs::read_to_string(&path).expect("saved"), port.number().to_string());
}

#[test]
fn a_garbled_file_is_ignored() {
    let (_dir, path) = file();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(&path, "not a port").expect("write");
    assert!(Port::remembered(&path).is_ok());
}
