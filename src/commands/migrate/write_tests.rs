//! What `migrate` and `--undo` do around the write itself: the backup, a file
//! edited meanwhile, a taken backup name.

use std::io;
use std::path::Path;

use super::fixtures::{
    INSTALL_SH, INSTALL_SH_MIGRATED, STAMP, Setup, checkers, home_env, migrate, read,
};
use crate::error::NvmExitCode;
use crate::fakes::FakeFileSystem;
use crate::ports::{FileSystem, Prompt};

const BASHRC: &str = "/Users/u/.bashrc";
const EDIT: &str = "alias ll='ls -l'\n";

/// Someone who edits `path` while the question waits, then answers yes.
struct EditingPrompt<'a> {
    fs: &'a FakeFileSystem,
    path: &'a str,
}

impl Prompt for EditingPrompt<'_> {
    fn confirm(&self, _question: &str) -> io::Result<bool> {
        let edited = format!("{}{EDIT}", read(self.fs, self.path));
        self.fs.write_file(Path::new(self.path), &edited)?;
        Ok(true)
    }
}

fn run_editing(fs: &FakeFileSystem, args: &[&str]) -> crate::commands::Output {
    let process = checkers();
    let prompt = EditingPrompt { fs, path: BASHRC };
    let env = home_env();
    let setup = Setup {
        fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    setup.output(args)
}

fn backup(sequence: &str) -> String {
    format!("{BASHRC}{STAMP}{sequence}")
}

#[test]
fn a_file_edited_while_the_question_waits_is_left_alone() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let output = run_editing(&fs, &[]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!(
            "nvm migrate: {BASHRC}: changed since the diff was shown; left unchanged: run it again"
        )
    );
    assert_eq!(read(&fs, BASHRC), format!("{INSTALL_SH}{EDIT}"));
    assert!(!fs.is_file(Path::new(&backup(""))));
}

#[test]
fn an_undo_of_a_file_edited_while_the_question_waits_is_refused() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH_MIGRATED)
        .with_file(&backup(""), INSTALL_SH);
    let output = run_editing(&fs, &["--undo"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(output.stderr.contains("changed since the diff was shown"));
    assert_eq!(read(&fs, BASHRC), format!("{INSTALL_SH_MIGRATED}{EDIT}"));
    assert!(!fs.is_file(Path::new(&backup("-1"))));
}

#[test]
fn a_file_refused_by_its_check_leaves_no_backup() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let process =
        checkers()
            .with_failure("bash")
            .with_run("bash", &format!("-n {BASHRC}"), true, "");
    let prompt = crate::fakes::FakePrompt::unavailable();
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["--yes"]).status, NvmExitCode::Failure);
    assert!(!fs.is_file(Path::new(&backup(""))));
    assert_eq!(
        migrate(&fs, &["--undo", "--yes"]).stderr,
        "nvm migrate: no backup found"
    );
}

#[test]
fn a_taken_backup_name_gets_the_next_sequence_number() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH)
        .with_file(&backup(""), "older\n");
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains(&format!("(backup: {})", backup("-1")))
    );
    assert_eq!(read(&fs, &backup("-1")), INSTALL_SH);
    assert_eq!(read(&fs, &backup("")), "older\n");
}

#[test]
fn undo_backs_the_current_content_up_so_the_undo_can_be_undone() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    migrate(&fs, &["--yes"]);
    let edited = format!("{INSTALL_SH_MIGRATED}{EDIT}");
    fs.write_file(Path::new(BASHRC), &edited).unwrap();
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(output.stdout.ends_with(&format!(
        "restored {BASHRC} from {} (backup: {})",
        backup(""),
        backup("-1")
    )));
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert_eq!(read(&fs, &backup("-1")), edited);
    migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(read(&fs, BASHRC), edited);
}
