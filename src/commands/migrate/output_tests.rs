//! What `migrate` prints of a profile it cannot trust: raw control bytes
//! and files that are not text.

use std::path::Path;

use super::fixtures::{INSTALL_SH, migrate, read};
use crate::error::NvmExitCode;
use crate::fakes::FakeFileSystem;
use crate::ports::FileSystem;

const BASHRC: &str = "/Users/u/.bashrc";

#[test]
fn control_bytes_of_the_profile_are_shown_in_caret_notation() {
    let text = format!("PS1='\x1b[31m$ \x1b[0m'\x07\n{INSTALL_SH}");
    let fs = FakeFileSystem::default().with_file(BASHRC, &text);
    let output = migrate(&fs, &["--dry-run"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(
        output.stdout.contains(" PS1='^[[31m$ ^[[0m'^G\n"),
        "{}",
        output.stdout
    );
    assert!(!output.stdout.contains(['\x1b', '\x07']));
}

#[test]
fn a_profile_that_is_not_utf8_is_reported_and_fails_the_run() {
    let fs = FakeFileSystem::default();
    let mut bytes = b"# caf\xe9\n".to_vec();
    bytes.extend_from_slice(INSTALL_SH.as_bytes());
    fs.write_bytes(Path::new(BASHRC), &bytes).unwrap();
    let zshrc = "/Users/u/.zshrc";
    fs.write_file(Path::new(zshrc), INSTALL_SH).unwrap();
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(
        output.stderr.starts_with(&format!(
            "nvm migrate: warning: {BASHRC}: not read (stream did not contain valid UTF-8); \
not migrated"
        )),
        "{}",
        output.stderr
    );
    assert!(read(&fs, zshrc).contains("nvmrc init zsh"));
}
