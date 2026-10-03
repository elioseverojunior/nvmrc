//! End-to-end: `install` and `uninstall` with the real binary, a real
//! temporary `$NVM_DIR` and a mirror served on a local port that holds a real
//! `.tar.gz`.

mod common;

use std::fs;
use std::process::Command;

use common::{mirror, nvm, slug, stderr, stdout};

#[test]
fn install_puts_a_working_version_in_place_and_ls_sees_it() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Downloading and installing node v20.10.0...\n\
         Creating default alias: default -> 20 (-> v20.10.0 *)\n"
    );
    assert!(stderr(&output).contains("Checksums matched!"));
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_eq!(
        fs::read_to_string(&node).unwrap(),
        "#!/bin/sh\necho v20.10.0\n"
    );
    let listed = nvm(home.path(), &mirror, &["ls", "--no-alias"]);
    assert_eq!(stdout(&listed), "       v20.10.0 *\n");
    assert!(
        !home
            .path()
            .join(".cache/bin")
            .join(slug("v20.10.0").unwrap())
            .join("files")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn the_installed_node_can_run() {
    use std::os::unix::fs::PermissionsExt;
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_ne!(fs::metadata(&node).unwrap().permissions().mode() & 0o111, 0);
    let ran = Command::new(&node).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "v20.10.0\n");
}

#[test]
fn a_second_install_says_so_and_a_cached_archive_is_reused_after_uninstall() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let again = nvm(home.path(), &mirror, &["install", "v20.10.0"]);
    assert_eq!(stderr(&again), "v20.10.0 is already installed.\n");
    assert_eq!(stdout(&again), "");
    let removed = nvm(home.path(), &mirror, &["uninstall", "20"]);
    assert_eq!(stdout(&removed), "Uninstalled node v20.10.0\n");
    assert!(!home.path().join("versions/node/v20.10.0").exists());
    let reinstalled = nvm(home.path(), &mirror, &["install", "20"]);
    assert!(stderr(&reinstalled).contains("Checksums match! Using existing downloaded archive"));
    assert!(
        home.path()
            .join("versions/node/v20.10.0/bin/node")
            .is_file()
    );
}

#[test]
fn a_wrong_checksum_installs_nothing_and_exits_2() {
    let Some(mirror) = mirror(Some("0000")) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "-b", "20"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Checksums do not match:"));
    assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
    assert!(!home.path().join("versions").exists());
}

#[test]
fn a_version_that_is_not_on_the_mirror_exits_3() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "99"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        stderr(&output),
        "Version '99' not found - try `nvm ls-remote` to browse available versions.\n"
    );
}

#[test]
fn install_lts_makes_the_default_point_at_lts_star() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "--lts"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("Installing latest LTS version.\n"));
    assert_eq!(
        fs::read_to_string(home.path().join("alias/default")).unwrap(),
        "lts/*\n"
    );
}
