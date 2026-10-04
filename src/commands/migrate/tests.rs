use std::path::Path;

use super::fixtures::{
    HOME, INSTALL_SH, INSTALL_SH_MIGRATED, QUESTION, STAMP, Setup, checkers, home_env, migrate,
    read,
};
use crate::error::NvmExitCode;
use crate::fakes::{FakeFileSystem, FakePrompt};
use crate::ports::FileSystem;

const BASHRC: &str = "/Users/u/.bashrc";

fn install_sh() -> FakeFileSystem {
    FakeFileSystem::default().with_file(BASHRC, INSTALL_SH)
}

const INSTALL_SH_DIFF: &str = "--- a//Users/u/.bashrc\n\
+++ b//Users/u/.bashrc\n\
@@ -1,3 +1,6 @@\n \
export NVM_DIR=\"$HOME/.nvm\"\n\
-[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
-[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n\
+# >>> nvmrc init >>>\n\
+eval \"$(nvmrc init bash)\"\n\
+# <<< nvmrc init <<<\n\
+# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
+# [nvmrc-migrated] [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

fn backup_of(path: &str) -> String {
    format!("{path}{STAMP}")
}

#[test]
fn the_install_script_lines_are_migrated_with_a_backup() {
    let fs = install_sh();
    let output = migrate(&fs, &["--yes"]);
    let backup = backup_of(BASHRC);
    let expected = format!(
        "{}\nmigrated {BASHRC} (backup: {backup})\nnvm migrate: done",
        INSTALL_SH_DIFF.trim_end()
    );
    assert_eq!(output.stdout, expected);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
    assert_eq!(read(&fs, &backup), INSTALL_SH);
    assert_eq!(fs.replaced(), [Path::new(BASHRC)]);
}

#[test]
fn the_edited_file_is_checked_with_its_temporary_path() {
    let fs = install_sh();
    let temporary = format!("-n {HOME}/.nvmrc-tmp-{}-0", std::process::id());
    let process = checkers()
        .with_failure("bash")
        .with_run("bash", &temporary, true, "");
    let prompt = FakePrompt::unavailable();
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["-y"]).status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn dry_run_prints_the_diff_and_writes_nothing() {
    let fs = install_sh();
    let output = migrate(&fs, &["--dry-run"]);
    assert_eq!(output.stdout, INSTALL_SH_DIFF.trim_end());
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(!fs.is_file(Path::new(&backup_of(BASHRC))));
    assert!(fs.replaced().is_empty());
}

fn with_answer(prompt: &FakePrompt) -> (FakeFileSystem, crate::commands::Output) {
    let fs = install_sh();
    let process = checkers();
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt,
    };
    let output = setup.output(&[]);
    (fs, output)
}

#[test]
fn a_yes_at_the_prompt_applies_and_the_question_shows_the_diff() {
    let prompt = FakePrompt::answering(true);
    let (fs, output) = with_answer(&prompt);
    assert_eq!(prompt.asked(), [format!("{INSTALL_SH_DIFF}{QUESTION}")]);
    let backup = backup_of(BASHRC);
    assert_eq!(
        output.stdout,
        format!("migrated {BASHRC} (backup: {backup})\nnvm migrate: done")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn a_no_at_the_prompt_aborts_without_writing() {
    let prompt = FakePrompt::answering(false);
    let (fs, output) = with_answer(&prompt);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "nvm migrate: aborted");
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(!fs.is_file(Path::new(&backup_of(BASHRC))));
}

#[test]
fn without_a_terminal_and_without_yes_it_shows_the_diff_and_fails() {
    let prompt = FakePrompt::unavailable();
    let (fs, output) = with_answer(&prompt);
    assert_eq!(output.stdout, INSTALL_SH_DIFF.trim_end());
    assert_eq!(
        output.stderr,
        "nvm migrate: confirmation needs a terminal: pass --yes"
    );
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
}

#[test]
fn yes_never_asks() {
    let fs = install_sh();
    let process = checkers();
    let prompt = FakePrompt::answering(false);
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["--yes"]).status, NvmExitCode::Success);
    assert!(prompt.asked().is_empty());
}

#[test]
fn a_second_run_has_nothing_to_migrate() {
    let fs = install_sh();
    migrate(&fs, &["--yes"]);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn no_profile_file_is_nothing_to_migrate() {
    let output = migrate(&FakeFileSystem::default(), &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn an_outdated_block_is_replaced() {
    let old = "export A=1\n# >>> nvmrc init >>>\neval \"$(nvm init bash)\"\n# <<< nvmrc init <<<\n";
    let fs = FakeFileSystem::default().with_file(BASHRC, old);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        read(&fs, BASHRC),
        "export A=1\n# >>> nvmrc init >>>\neval \"$(nvmrc init bash)\"\n# <<< nvmrc init <<<\n"
    );
    assert_eq!(read(&fs, &backup_of(BASHRC)), old);
}

#[test]
fn a_loader_in_a_sourced_file_is_left_for_a_manual_fix() {
    let sourcing = "source ~/.nvm-load.sh\n";
    let loader = "[ -s \"$HOME/.nvm/nvm.sh\" ] && \\. \"$HOME/.nvm/nvm.sh\"\n";
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, sourcing)
        .with_file("/Users/u/.nvm-load.sh", loader);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(
        output.stdout,
        "nvm migrate: nothing to migrate\n\
nvm migrate: 1 manual finding(s) left: run `nvm doctor`"
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), sourcing);
    assert_eq!(read(&fs, "/Users/u/.nvm-load.sh"), loader);
    assert!(fs.replaced().is_empty());
}

#[test]
fn a_current_block_for_another_shell_is_kept() {
    let profile = "# >>> nvmrc init >>>\neval \"$(nvmrc init sh)\"\n# <<< nvmrc init <<<\n";
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", profile);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(read(&fs, "/Users/u/.profile"), profile);
}
