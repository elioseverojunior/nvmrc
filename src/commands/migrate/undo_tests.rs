use std::path::Path;

use super::fixtures::{INSTALL_SH, INSTALL_SH_MIGRATED, STAMP, link, migrate, read};
use crate::domain::migration::unified_diff;
use crate::error::NvmExitCode;
use crate::fakes::FakeFileSystem;
use crate::ports::FileSystem;

const BASHRC: &str = "/Users/u/.bashrc";
const OLDER: &str = "/Users/u/.bashrc.nvmrc-backup-20260101T000000Z";
const NEWER: &str = "/Users/u/.bashrc.nvmrc-backup-20261001T000000Z";

fn with_two_backups() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH_MIGRATED)
        .with_file(OLDER, "older\n")
        .with_file(NEWER, INSTALL_SH)
        .with_file("/Users/u/.bashrc.nvmrc-backup-junk", "junk\n")
}

#[test]
fn undo_restores_the_latest_backup_and_keeps_the_backups() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo", "--yes"]);
    let diff = unified_diff(BASHRC, INSTALL_SH_MIGRATED, INSTALL_SH);
    assert_eq!(
        output.stdout,
        format!(
            "{}\nrestored {BASHRC} from {NEWER} (backup: {BASHRC}{STAMP})",
            diff.trim_end()
        )
    );
    assert_eq!(read(&fs, &format!("{BASHRC}{STAMP}")), INSTALL_SH_MIGRATED);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert_eq!(read(&fs, NEWER), INSTALL_SH);
    assert_eq!(read(&fs, OLDER), "older\n");
}

#[test]
fn undo_dry_run_only_prints_the_diff() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo", "--dry-run"]);
    let diff = unified_diff(BASHRC, INSTALL_SH_MIGRATED, INSTALL_SH);
    assert_eq!(output.stdout, diff.trim_end());
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
    assert!(fs.replaced().is_empty());
}

#[test]
fn undo_without_a_terminal_and_without_yes_fails() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn undo_without_any_backup_fails() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH_MIGRATED);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.stderr, "nvm migrate: no backup found");
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn undo_of_a_backup_equal_to_the_file_has_nothing_to_restore() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH)
        .with_file(NEWER, INSTALL_SH);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to restore");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(fs.replaced().is_empty());
}

#[test]
fn migrate_then_undo_through_a_symlink_restores_the_target() {
    let target = "/Users/u/dotfiles/.zshrc";
    let fs = FakeFileSystem::default().with_file(target, INSTALL_SH);
    link(&fs, "dotfiles/.zshrc", "/Users/u/.zshrc");
    migrate(&fs, &["--yes"]);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(read(&fs, target), INSTALL_SH);
    assert!(fs.read_link(Path::new("/Users/u/.zshrc")).is_ok());
}

#[test]
fn an_empty_backup_is_not_restored_over_a_file_with_content() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH_MIGRATED)
        .with_file(NEWER, "");
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: {NEWER}: the backup is empty; {BASHRC} was not restored")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}
