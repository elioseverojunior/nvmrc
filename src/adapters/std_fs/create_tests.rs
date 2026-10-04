use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::StdFileSystem;
use crate::ports::FileSystem;

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn create_new_file_copies_the_mode_of_a_private_original_through_a_link() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("zshrc");
    fs::write(&original, "x").unwrap();
    for bits in [0o600, 0o640, 0o755] {
        fs::set_permissions(&original, fs::Permissions::from_mode(bits)).unwrap();
        let link = root.path().join(format!("link-{bits:o}"));
        std::os::unix::fs::symlink(&original, &link).unwrap();
        let copy = root.path().join(format!("copy-{bits:o}"));
        StdFileSystem
            .create_new_file(&copy, "contents\n", &link)
            .unwrap();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "contents\n");
        assert_eq!(mode(&copy), bits);
    }
}

#[test]
fn create_new_file_is_private_when_the_original_is_missing() {
    let root = tempfile::tempdir().unwrap();
    let copy = root.path().join("copy");
    let missing = root.path().join("missing");
    StdFileSystem.create_new_file(&copy, "x", &missing).unwrap();
    assert_eq!(mode(&copy), 0o600);
}

#[test]
fn create_new_file_never_replaces_a_file_or_a_link() {
    let root = tempfile::tempdir().unwrap();
    let taken = root.path().join("taken");
    fs::write(&taken, "kept").unwrap();
    let dangling = root.path().join("dangling");
    std::os::unix::fs::symlink(root.path().join("nowhere"), &dangling).unwrap();
    for path in [&taken, &dangling] {
        let error = StdFileSystem
            .create_new_file(path, "new", &taken)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    }
    assert_eq!(fs::read_to_string(&taken).unwrap(), "kept");
    assert!(!root.path().join("nowhere").exists());
}
