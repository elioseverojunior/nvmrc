use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

const OLD_NPM: &str = "/n/versions/node/v18.19.0/bin/npm";
const NEW_NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn fs() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable("/n/versions/node/v18.19.0/bin/node", "")
        .with_executable(OLD_NPM, "")
        .with_executable("/n/versions/node/v20.10.0/bin/node", "")
        .with_executable(NEW_NPM, "")
}

fn run_with(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    path: &str,
    command: &str,
    line: &str,
) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    run(
        &Context::new(fs, &env).with_process(process),
        command,
        &words,
    )
}

fn active_20() -> &'static str {
    "/n/versions/node/v20.10.0/bin:/usr/bin"
}

/// The expectations are what the real `nvm reinstall-packages` printed.
#[test]
fn it_reinstalls_the_packages_of_another_version_into_the_one_in_use() {
    let process = FakeProcess::default()
        .with_success(OLD_NPM, "list -g --depth=0", "/x\n├── yarn@1.22.19\n")
        .with_success(NEW_NPM, "install -g --quiet yarn@1.22.19", "added\n");
    let output = run_with(&fs(), &process, active_20(), "reinstall-packages", "18").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stdout,
        "Reinstalling global packages from v18.19.0...\nadded\nLinking global packages from v18.19.0...\nNo linked global packages found..."
    );
}

#[test]
fn the_version_in_use_cannot_be_its_own_source() {
    let process = FakeProcess::default();
    for word in ["20", "v20.10.0"] {
        let output = run_with(&fs(), &process, active_20(), "reinstall-packages", word).unwrap();
        assert_eq!(output.status, NvmExitCode::MissingTarget);
        assert_eq!(
            output.stderr,
            "Can not reinstall packages from the current version of node."
        );
    }
}

#[test]
fn an_unknown_version_prints_the_use_message_and_finds_nothing_with_status_0() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        active_20(),
        "reinstall-packages",
        "99",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stderr
            .starts_with("N/A: version \"N/A\" is not yet installed.")
    );
}

#[test]
fn without_one_word_the_usage_names_the_command_used() {
    for command in ["reinstall-packages", "copy-packages"] {
        for line in ["", "18 20"] {
            let error = run_with(&fs(), &FakeProcess::default(), "", command, line).unwrap_err();
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
            assert_eq!(
                error.to_string(),
                format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.")
            );
        }
    }
}

#[test]
fn without_an_npm_in_use_there_is_nowhere_to_install() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        "/usr/bin",
        "reinstall-packages",
        "18",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(output.stderr.starts_with("npm was not found on PATH"));
}

#[test]
fn system_needs_a_system_node() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        active_20(),
        "reinstall-packages",
        "system",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "No system version of node or io.js detected."
    );
}

#[test]
fn system_reinstalls_from_the_npm_of_the_system_node() {
    let with_system = fs()
        .with_executable("/usr/bin/node", "")
        .with_executable("/usr/bin/npm", "");
    let process = FakeProcess::default()
        .with_success(
            "/usr/bin/npm",
            "list -g --depth=0",
            "/x\n├── yarn@1.22.19\n",
        )
        .with_success(NEW_NPM, "install -g --quiet yarn@1.22.19", "");
    let output = run_with(
        &with_system,
        &process,
        active_20(),
        "reinstall-packages",
        "system",
    )
    .unwrap();
    assert!(
        output
            .stdout
            .starts_with("Reinstalling global packages from system...")
    );
}
