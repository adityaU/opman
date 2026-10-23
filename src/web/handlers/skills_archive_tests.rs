use super::*;
use std::io::Write;

fn archive(path: &str) -> anyhow::Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file(path, zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"# Skill")?;
    Ok(zip.finish()?.into_inner())
}

#[test]
fn imports_skill_archives_and_rejects_traversal() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    assert_eq!(
        extract(&archive("example/SKILL.md")?, dir.path()).map_err(anyhow::Error::msg)?,
        1
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("example/SKILL.md"))?,
        "# Skill"
    );
    assert!(extract(&archive("../outside")?, dir.path()).is_err());
    assert!(extract(&archive("/outside")?, dir.path()).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn zip_import_never_follows_existing_symlinks() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::os::unix::fs::symlink(outside.path(), dir.path().join("linked"))?;
    assert!(extract(&archive("linked/SKILL.md")?, dir.path()).is_err());
    assert!(!outside.path().join("SKILL.md").exists());
    Ok(())
}
