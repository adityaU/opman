//! Unpacking keeps execute bits and refuses entries that leave the install folder; the
//! Antigravity row resolves to the unpacked binary and ships switched off.

use std::io::Write;

use super::*;

fn zip_with(entries: &[(&str, u32, &[u8])]) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    let mut writer = zip::ZipWriter::new(file.reopen().expect("reopen"));
    for (name, mode, body) in entries {
        let options = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
        writer.start_file(*name, options).expect("start entry");
        writer.write_all(body).expect("write entry");
    }
    writer.finish().expect("finish zip");
    file
}

#[cfg(unix)]
#[test]
fn unpacking_keeps_the_execute_bit() {
    use std::os::unix::fs::PermissionsExt;
    let zip = zip_with(&[
        ("agy_acp_server.par", 0o775, b"#!/bin/sh\n"),
        ("notes.txt", 0o644, b"hi"),
    ]);
    let into = tempfile::tempdir().expect("temp dir");
    let target = into.path().join("out");
    unpack(zip.path(), &target).expect("unpacks");
    let mode = |name: &str| {
        std::fs::metadata(target.join(name))
            .expect("exists")
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("agy_acp_server.par"), 0o775);
    assert_eq!(mode("notes.txt"), 0o644);
}

#[test]
fn an_entry_that_escapes_the_folder_is_refused() {
    let zip = zip_with(&[("../evil", 0o644, b"x")]);
    let into = tempfile::tempdir().expect("temp dir");
    assert!(unpack(zip.path(), &into.path().join("out")).is_err());
}

#[test]
fn antigravity_resolves_to_the_unpacked_binary_and_ships_off() {
    let archive =
        super::super::catalog::archive_for("antigravity").expect("antigravity ships as an archive");
    let entry = super::super::catalog::ENTRIES
        .iter()
        .find(|entry| entry.id == "antigravity")
        .expect("catalogue row");
    let config = entry.config();
    assert!(!config.enabled, "a downloaded agent must be opted into");
    match archive.build() {
        Some(build) => {
            let dir = archive.install_dir("antigravity").expect("data dir");
            assert_eq!(std::path::Path::new(&config.command), dir.join(build.cmd));
            assert!(config.command.contains("antigravity"));
            assert!(config.command.contains(archive.version));
        }
        // No build for this machine: unlaunchable, never a guessed binary.
        None => assert!(config.command.is_empty()),
    }
}

#[test]
fn other_agents_are_not_archives() {
    assert!(super::super::catalog::archive_for("codex").is_none());
    assert!(super::super::catalog::archive_for("nope").is_none());
}
