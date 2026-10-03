//! Digest 7.3: one test per observed row of `nvm run`, which plans `node`
//! (or `iojs`) through `nvm exec`.

use crate::commands::Output;
use crate::commands::exec::tests::{
    ACTIVE_18, BASE_PATH, CURRENT_NOT_INSTALLED, N18, N20, NO_NVMRC, WARNING, assert_runs,
    assert_stops, assert_streams, child, child_on, env_on, lab, npm_versions, strings,
};
use crate::commands::run::run;
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};

const RUNNING_18: &str = "Running node v18.20.4 (npm v10.7.0)";
const N22: &str = "/n/versions/node/v22.3.0";
const RUN_WARNING: &str = "WARNING: `nvm run` was invoked without a version argument \
and without an .nvmrc file.\n  Falling back to the active node version; this will become an \
error in a future release.\n  Pass `current` explicitly (e.g. `nvm run current ...`) to \
silence this warning.";
const USAGE: &str = "Usage: nvm run [<version>] [<args>]\n  Provide a <version>, or run from \
a directory containing an .nvmrc file.\n  Run `nvm --help` for full help.";
const INSTALL_FROM_NVMRC: &str = "You need to run `nvm install` to install and use the node \
version specified in `.nvmrc`.";

fn run_in(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Output {
    let process = npm_versions();
    let context = Context::new(fs, env).with_process(&process);
    run(&context, &strings(args)).expect("nvm run runs")
}

fn run_args(args: &[&str]) -> Output {
    run_in(&lab(), &env_on(BASE_PATH), args)
}

fn with_nvmrc(version: &str) -> FakeFileSystem {
    lab().with_file("/proj/.nvmrc", &format!("{version}\n"))
}

/// `node` and `args`: the command `nvm run` plans.
fn node<'a>(args: &[&'a str]) -> Vec<&'a str> {
    [&["node"][..], args].concat()
}

fn none_to_run() -> String {
    format!(
        "{NO_NVMRC}\n{RUN_WARNING}\nN/A: version \"none\" is not yet installed.\n\n{INSTALL_FROM_NVMRC}"
    )
}

#[test]
fn run_18_version_runs_node_with_the_arguments() {
    let output = run_args(&["18", "--version"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &node(&["--version"])),
    );
}

#[test]
fn run_18_exit_3_runs_node_whose_status_is_passed_on() {
    let output = run_args(&["18", "exit", "3"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &node(&["exit", "3"])),
    );
}

#[test]
fn run_silent_prints_nothing_before_node() {
    let output = run_args(&["--silent", "18", "a", "b"]);
    assert_runs(&output, "", child(N18, "v18.20.4", &node(&["a", "b"])));
}

#[test]
fn run_options_after_the_version_go_to_node() {
    let output = run_args(&["18", "--silent", "a"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &node(&["--silent", "a"])),
    );
}

#[test]
fn run_lts_runs_the_latest_lts() {
    let output = run_args(&["--lts", "a"]);
    let stdout = "Running node latest LTS -> v20.11.1 (npm v10.2.4)";
    assert_runs(&output, stdout, child(N20, "lts/*", &node(&["a"])));
}

#[test]
fn run_lts_hydrogen_runs_that_line() {
    let output = run_args(&["--lts=hydrogen", "a"]);
    let stdout = "Running node LTS \"hydrogen\" -> v18.20.4 (npm v10.7.0)";
    assert_runs(&output, stdout, child(N18, "lts/hydrogen", &node(&["a"])));
}

#[test]
fn run_lts_argon_is_not_installed() {
    let output = run_args(&["--lts=argon", "a"]);
    let stderr = "N/A: version \"lts/argon\" is not yet installed.\n\n\
You need to run `nvm install lts/argon` to install and use it.";
    assert_stops(&output, "", stderr, NvmExitCode::Failure);
}

#[test]
fn run_99_is_not_installed() {
    let output = run_args(&["99", "a"]);
    let stderr = "N/A: version \"v99\" is not yet installed.\n\n\
You need to run `nvm install 99` to install and use it.";
    assert_stops(&output, "", stderr, NvmExitCode::Failure);
}

#[test]
fn run_a_script_with_nothing_active_and_no_nvmrc_gives_the_nvmrc_hint_for_none() {
    for args in [&["app.js"][..], &["--", "18", "x"], &["missing", "x"]] {
        assert_stops(&run_args(args), "", &none_to_run(), NvmExitCode::Failure);
    }
}

#[test]
fn run_a_script_with_18_active_warns_and_runs_it_on_18() {
    let output = run_in(&lab(), &env_on(ACTIVE_18), &["app.js", "x"]);
    let stderr = format!("{NO_NVMRC}\n{RUN_WARNING}");
    assert_streams(&output, RUNNING_18, &stderr, NvmExitCode::Success);
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &node(&["app.js", "x"]));
    assert_eq!(output.spawn, Some(expected));
}

#[test]
fn run_silent_a_script_with_18_active_prints_nothing() {
    let output = run_in(&lab(), &env_on(ACTIVE_18), &["--silent", "app.js", "x"]);
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &node(&["app.js", "x"]));
    assert_runs(&output, "", expected);
}

#[test]
fn run_alone_without_an_nvmrc_is_the_usage_with_127() {
    let output = run_args(&[]);
    assert_stops(
        &output,
        "",
        &format!("{NO_NVMRC}\n{USAGE}"),
        NvmExitCode::NotFound,
    );
}

#[test]
fn run_silent_alone_without_an_nvmrc_is_only_the_usage() {
    let output = run_args(&["--silent"]);
    assert_stops(&output, "", USAGE, NvmExitCode::NotFound);
}

#[test]
fn run_iojs_runs_iojs() {
    let output = run_args(&["iojs", "x"]);
    let directory = "/n/versions/io.js/v3.3.1";
    let stdout = "Running io.js v3.3.1 (npm v2.14.3)";
    assert_runs(
        &output,
        stdout,
        child(directory, "iojs-v3.3.1", &["iojs", "x"]),
    );
}

#[test]
fn run_node_and_22_run_the_newest_node() {
    for version in ["node", "22"] {
        let output = run_args(&[version, "x"]);
        assert_runs(
            &output,
            "Running node v22.3.0",
            child(N22, "v22.3.0", &node(&["x"])),
        );
    }
}

#[test]
fn run_a_double_dash_after_the_version_goes_to_node() {
    let output = run_args(&["18", "--", "x"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &node(&["--", "x"])),
    );
}

#[test]
fn run_empty_arguments_before_the_version_are_skipped() {
    let output = run_args(&["", "18", "x"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &node(&["x"])));
}

#[test]
fn run_a_looping_alias_falls_to_exec_which_takes_it_for_a_command() {
    let output = run_args(&["loopa", "x"]);
    let stderr = format!("{NO_NVMRC}\n{WARNING}\n{CURRENT_NOT_INSTALLED}");
    assert_stops(&output, "", &stderr, NvmExitCode::Failure);
}

#[test]
fn run_current_with_18_active_runs_on_it() {
    let output = run_in(&lab(), &env_on(ACTIVE_18), &["current", "x"]);
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &node(&["x"]));
    assert_runs(&output, RUNNING_18, expected);
}

#[test]
fn run_passes_the_help_words_to_node() {
    // DELIBERATE DEVIATION: nvm.sh prints its help for `nvm run 18 -h`.
    let output = run_args(&["18", "-h"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &node(&["-h"])));
}

#[test]
fn run_alone_with_an_nvmrc_runs_node_on_its_version() {
    let output = run_in(&with_nvmrc("20"), &env_on(BASE_PATH), &[]);
    let stdout = "Found '/proj/.nvmrc' with version <20>\nRunning node v20.11.1 (npm v10.2.4)";
    assert_runs(&output, stdout, child(N20, "v20.11.1", &node(&[])));
}

#[test]
fn run_a_script_with_an_nvmrc_runs_it_on_its_version() {
    let output = run_in(&with_nvmrc("20"), &env_on(BASE_PATH), &["app.js"]);
    let stdout = "Found '/proj/.nvmrc' with version <20>\nRunning node v20.11.1 (npm v10.2.4)";
    assert_runs(&output, stdout, child(N20, "v20.11.1", &node(&["app.js"])));
}

#[test]
fn run_silent_a_script_with_an_nvmrc_prints_nothing() {
    let output = run_in(
        &with_nvmrc("20"),
        &env_on(BASE_PATH),
        &["--silent", "app.js"],
    );
    assert_runs(&output, "", child(N20, "v20.11.1", &node(&["app.js"])));
}

#[test]
fn run_a_version_ignores_the_nvmrc() {
    let output = run_in(&with_nvmrc("20"), &env_on(BASE_PATH), &["18", "app.js"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &node(&["app.js"])),
    );
}

#[test]
fn run_a_script_with_an_nvmrc_naming_a_missing_version_gives_the_nvmrc_hint() {
    let output = run_in(&with_nvmrc("99"), &env_on(BASE_PATH), &["app.js"]);
    let stderr = format!("N/A: version \"v99\" is not yet installed.\n\n{INSTALL_FROM_NVMRC}");
    let stdout = "Found '/proj/.nvmrc' with version <99>";
    assert_stops(&output, stdout, &stderr, NvmExitCode::Failure);
}

#[test]
fn run_alone_with_an_nvmrc_naming_a_missing_version_is_the_usage() {
    let output = run_in(&with_nvmrc("99"), &env_on(BASE_PATH), &[]);
    let stdout = "Found '/proj/.nvmrc' with version <99>";
    assert_stops(&output, stdout, USAGE, NvmExitCode::NotFound);
}
