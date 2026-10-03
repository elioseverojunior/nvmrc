use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";
const PATH: &str = "/n/versions/node/v20.10.0";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn options(latest_npm: bool, skip: bool) -> Options {
    Options {
        latest_npm,
        skip_default_packages: skip,
        ..Options::default()
    }
}

fn run_steps(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    options: &Options,
) -> (NvmExitCode, String, String) {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(fs, &env).with_process(process);
    let mut transcript = Transcript::default();
    let status = run(
        &context,
        options,
        &version(),
        Path::new(PATH),
        &mut transcript,
    )
    .unwrap();
    let output = transcript.finish(status);
    (status, output.stdout, output.stderr)
}

fn world(default_packages: Option<&str>) -> FakeFileSystem {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    match default_packages {
        Some(contents) => fs.with_file("/n/default-packages", contents),
        None => fs,
    }
}

fn calls(process: &FakeProcess) -> Vec<String> {
    process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect()
}

/// The expectations are what `nvm install` of the real `nvm.sh` printed, and
/// the `npm` commands it ran.
#[test]
fn the_default_packages_are_installed_with_one_npm_command() {
    let fs = world(Some("# my packages\nyarn\n\n@scope/tool\n"));
    let process =
        FakeProcess::default().with_success(NPM, "install -g --quiet yarn @scope/tool", "added\n");
    let (status, stdout, _) = run_steps(&fs, &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Installing default global packages from /n/default-packages...\n\
         npm install -g --quiet  yarn @scope/tool\n\
         added"
    );
}

#[test]
fn skip_default_packages_leaves_the_file_alone() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default();
    let (status, stdout, _) = run_steps(&fs, &process, &options(false, true));
    assert_eq!((status, stdout.as_str()), (NvmExitCode::Success, ""));
    assert!(calls(&process).is_empty());
}

#[test]
fn no_default_packages_file_or_an_empty_one_runs_nothing() {
    let process = FakeProcess::default();
    for fs in [world(None), world(Some("# nothing\n"))] {
        let (status, stdout, _) = run_steps(&fs, &process, &options(false, false));
        assert_eq!((status, stdout.as_str()), (NvmExitCode::Success, ""));
    }
    assert!(calls(&process).is_empty());
}

#[test]
fn a_line_with_two_values_is_status_1_and_names_the_file() {
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&world(Some("a b\n")), &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stderr,
        "Only one package per line is allowed in `/n/default-packages`. Please remove any lines with multiple space-separated values."
    );
}

#[test]
fn a_failed_package_install_is_status_1_with_the_hint() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: "E404\n".to_owned(),
    };
    let process = FakeProcess::default().with_execution(NPM, "install -g --quiet yarn", failed);
    let (status, _, stderr) = run_steps(&world(Some("yarn\n")), &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stderr,
        "E404\nFailed installing default packages. Please check if your default-packages file or a package in it has problems!"
    );
}

#[test]
fn latest_npm_runs_before_the_default_packages() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "")
        .with_success(NPM, "install -g --quiet yarn", "");
    let (status, stdout, _) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::Success);
    assert!(stdout.starts_with("Attempting to upgrade to the latest working version of npm..."));
    assert!(stdout.contains("npm install -g --quiet yarn"));
    let ran = calls(&process);
    let upgrade = ran
        .iter()
        .position(|call| call == "install -g npm@10")
        .unwrap();
    let packages = ran
        .iter()
        .position(|call| call == "install -g --quiet yarn")
        .unwrap();
    assert!(upgrade < packages);
}

#[test]
fn a_failed_upgrade_stops_before_the_default_packages() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::MissingTarget);
    assert_eq!(stderr, "Unable to obtain npm version.");
    assert!(!calls(&process).iter().any(|call| call.contains("yarn")));
}

#[test]
fn a_version_without_npm_skips_the_steps_with_a_warning_and_never_fails() {
    let fs = FakeFileSystem::default().with_file("/n/default-packages", "yarn\n");
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stderr,
        "npm was not found in v20.10.0; skipping the npm upgrade.\nnpm was not found in v20.10.0; skipping the default packages."
    );
    assert!(process.executed().is_empty());
}
