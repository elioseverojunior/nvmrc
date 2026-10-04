use std::path::Path;

use super::fixtures::{INSTALL_SH, STAMP, Setup, checkers, home_env, link, migrate, read};
use crate::error::NvmExitCode;
use crate::fakes::{FakeFileSystem, FakeProcess, FakePrompt};
use crate::ports::FileSystem;

const ZSHRC: &str = "/Users/u/.zshrc";
const TARGET: &str = "/Users/u/dotfiles/zsh/.zshrc";
const BASHRC: &str = "/Users/u/.bashrc";

fn run_with(fs: &FakeFileSystem, process: &FakeProcess) -> crate::commands::Output {
    let prompt = FakePrompt::unavailable();
    let env = home_env();
    let setup = Setup {
        fs,
        env: &env,
        process,
        prompt: &prompt,
    };
    setup.output(&["--yes"])
}

#[test]
fn a_symlinked_zshrc_is_written_through_the_link_with_the_backup_beside_the_link() {
    let fs = FakeFileSystem::default().with_file(TARGET, INSTALL_SH);
    link(&fs, "dotfiles/zsh/.zshrc", ZSHRC);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(
        fs.read_link(Path::new(ZSHRC)).unwrap(),
        Path::new("dotfiles/zsh/.zshrc")
    );
    let backup = format!("{ZSHRC}{STAMP}");
    assert!(
        output
            .stdout
            .contains(&format!("migrated {ZSHRC} (backup: {backup})"))
    );
    assert_eq!(read(&fs, &backup), INSTALL_SH);
    assert!(fs.read_link(Path::new(&backup)).is_err());
    assert!(read(&fs, TARGET).contains("eval \"$(nvmrc init zsh)\"\n"));
    assert!(!fs.is_file(Path::new(&format!("{TARGET}{STAMP}"))));
}

#[test]
fn a_file_that_fails_its_check_is_refused_and_the_others_are_migrated() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH)
        .with_file(ZSHRC, INSTALL_SH);
    let process = checkers().with_failure("bash");
    let output = run_with(&fs, &process);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: {BASHRC}: the edited file would not pass `bash -n`; left unchanged")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(read(&fs, ZSHRC).contains("nvmrc init zsh"));
    assert!(output.stdout.contains(&format!("migrated {ZSHRC} ")));
    assert!(!output.stdout.contains(&format!("migrated {BASHRC} ")));
    assert_eq!(fs.replaced(), [Path::new(ZSHRC)]);
}

#[test]
fn a_missing_checker_is_noted_and_the_file_is_still_migrated() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let output = run_with(&fs, &FakeProcess::default());
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: bash not found: the syntax of {BASHRC} was not checked")
    );
    assert!(read(&fs, BASHRC).contains("nvmrc init bash"));
}

#[test]
fn shell_overrides_the_shell_of_the_block() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", INSTALL_SH);
    let output = migrate(&fs, &["--yes", "--shell=ksh"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(read(&fs, "/Users/u/.profile").contains("eval \"$(nvmrc init ksh)\"\n"));
}

#[test]
fn the_profile_takes_the_login_shell_of_shell() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", INSTALL_SH);
    let process = checkers();
    let prompt = FakePrompt::unavailable();
    let env = home_env().with_var("SHELL", "/bin/zsh");
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["--yes"]).status, NvmExitCode::Success);
    assert!(read(&fs, "/Users/u/.profile").contains("eval \"$(nvmrc init zsh)\"\n"));
}

#[test]
fn manual_findings_left_after_a_migration_are_counted() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, &format!("{INSTALL_SH}source ~/.nvm-load.sh\n"))
        .with_file("/Users/u/.nvm-load.sh", "source ~/.nvm/nvm.sh\n");
    let output = migrate(&fs, &["--yes"]);
    assert!(
        output.stdout.ends_with(
            "nvm migrate: done\nnvm migrate: 1 manual finding(s) left: run `nvm doctor`"
        )
    );
    assert_eq!(output.status, NvmExitCode::Success);
}
