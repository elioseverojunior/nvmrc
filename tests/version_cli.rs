//! End-to-end: the real `nvm` binary against a real temporary `$NVM_DIR`.

use std::fs;
use std::process::Command;

fn nvm(nvm_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .output()
        .expect("run nvm")
}

#[test]
fn version_resolves_installed_versions_and_aliases() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("versions/node/v20.1.0/bin")).unwrap();
    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/default"), "v20\n").unwrap();

    let output = nvm(dir.path(), &["version", "default"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "v20.1.0\n");

    let missing = nvm(dir.path(), &["version", "16"]);
    assert_eq!(missing.status.code(), Some(3));
    assert_eq!(String::from_utf8_lossy(&missing.stderr), "N/A\n");
}
