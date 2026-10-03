use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

fn run_with(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    path: &str,
    args: &[&str],
) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_DEBUG", "1")
        .with_var("PATH", path);
    let words: Vec<String> = args.iter().map(|word| (*word).to_owned()).collect();
    run(&Context::new(fs, &env).with_process(process), &words)
}

#[test]
fn it_upgrades_the_npm_of_the_version_in_use() {
    let fs = FakeFileSystem::default()
        .with_executable("/n/versions/node/v20.10.0/bin/node", "")
        .with_executable("/n/versions/node/v20.10.0/bin/npm", "");
    let process = FakeProcess::default().with_success(
        "/n/versions/node/v20.10.0/bin/npm",
        "--version",
        "10.2.3\n",
    );
    let output = run_with(&fs, &process, "/n/versions/node/v20.10.0/bin:/usr/bin", &[]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stdout
            .contains("Detected node version v20.10.0, npm version v10.2.3")
    );
    assert!(output.stdout.contains("npm install -g npm@10"));
}

#[test]
fn a_system_node_is_asked_for_its_version() {
    let fs = FakeFileSystem::default()
        .with_executable("/usr/bin/node", "")
        .with_executable("/usr/bin/npm", "");
    let process = FakeProcess::default()
        .with_output("/usr/bin/node", "v18.19.0\n")
        .with_success("/usr/bin/npm", "--version", "10.2.3\n");
    let output = run_with(&fs, &process, "/usr/bin", &[]).unwrap();
    assert!(
        output
            .stdout
            .contains("Detected node version v18.19.0, npm version v10.2.3")
    );
}

#[test]
fn no_node_is_status_1_and_no_npm_status_2() {
    let (fs, process) = (FakeFileSystem::default(), FakeProcess::default());
    assert_eq!(
        run_with(&fs, &process, "", &[]).unwrap().status,
        NvmExitCode::Failure
    );
    let fs = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "");
    let output = run_with(&fs, &process, "/n/versions/node/v20.10.0/bin", &[]).unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
}

#[test]
fn any_word_is_the_usage_with_status_127() {
    let (fs, process) = (FakeFileSystem::default(), FakeProcess::default());
    let error = run_with(&fs, &process, "", &["x"]).unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    assert_eq!(
        error.to_string(),
        "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help."
    );
}
