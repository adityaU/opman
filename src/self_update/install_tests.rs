use super::*;
use flate2::write::GzEncoder;
use flate2::Compression;

fn archive_with(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
    let path = dir.join("a.tar.gz");
    let mut tar = tar::Builder::new(GzEncoder::new(
        File::create(&path).expect("valid test fixture"),
        Compression::fast(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, name, body)
        .expect("valid test fixture");
    tar.into_inner()
        .expect("valid test fixture")
        .finish()
        .expect("valid test fixture");
    path
}

#[test]
fn extracts_the_binary_and_makes_it_executable() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let archive = archive_with(dir.path(), "opman", b"#!bin");
    let out = dir.path().join("out");
    extract_binary(&archive, &out).expect("valid test fixture");
    assert_eq!(std::fs::read(&out).expect("valid test fixture"), b"#!bin");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&out)
                .expect("valid test fixture")
                .permissions()
                .mode()
                & 0o111,
            0o111
        );
    }
}

#[test]
fn refuses_an_archive_without_the_binary() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let archive = archive_with(dir.path(), "README", b"hi");
    let err =
        extract_binary(&archive, &dir.path().join("out")).expect_err("invalid fixture must fail");
    assert!(err.to_string().contains("does not contain"));
}

#[test]
fn refuses_a_file_that_is_not_an_archive() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let junk = dir.path().join("junk");
    std::fs::write(&junk, b"not gzip").expect("valid test fixture");
    assert!(extract_binary(&junk, &dir.path().join("out")).is_err());
}

#[test]
fn replace_swaps_the_file_in_place() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let exe = dir.path().join("opman");
    let new = dir.path().join("new");
    std::fs::write(&exe, b"old").expect("valid test fixture");
    std::fs::write(&new, b"new").expect("valid test fixture");
    replace(&exe, &new).expect("valid test fixture");
    assert_eq!(std::fs::read(&exe).expect("valid test fixture"), b"new");
    assert!(!new.exists());
}

#[test]
fn staging_files_are_removed_on_drop() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    let exe = dir.path().join("opman");
    let staging = Staging::beside(&exe).expect("valid test fixture");
    std::fs::write(&staging.archive, b"x").expect("valid test fixture");
    std::fs::write(&staging.binary, b"x").expect("valid test fixture");
    let (a, b) = (staging.archive.clone(), staging.binary.clone());
    drop(staging);
    assert!(!a.exists() && !b.exists());
}

#[test]
fn simultaneous_staging_directories_are_private_and_distinct() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let exe = dir.path().join("opman");
    let first = Staging::beside(&exe)?;
    let second = Staging::beside(&exe)?;
    assert_ne!(first.directory, second.directory);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&first.directory)?.permissions().mode() & 0o777,
            0o700
        );
    }
    let directory = first.directory.clone();
    drop(first);
    assert!(!directory.exists());
    Ok(())
}

#[test]
fn extraction_does_not_overwrite_existing_files() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let archive = archive_with(dir.path(), "opman", b"new");
    let out = dir.path().join("existing");
    std::fs::write(&out, b"old")?;
    assert!(extract_binary(&archive, &out).is_err());
    assert_eq!(std::fs::read(&out)?, b"old");
    Ok(())
}

#[test]
fn extraction_rejects_a_symlink_named_opman() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("link.tar.gz");
    let mut tar = tar::Builder::new(GzEncoder::new(File::create(&archive)?, Compression::fast()));
    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_mode(0o777);
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_link_name("outside")?;
    header.set_cksum();
    tar.append_data(&mut header, "opman", std::io::empty())?;
    tar.into_inner()?.finish()?;
    let output = dir.path().join("output");
    assert!(extract_binary(&archive, &output).is_err());
    assert!(!output.exists());
    Ok(())
}
