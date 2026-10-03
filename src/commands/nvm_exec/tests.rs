//! Digest 8: one test per observed row of the `nvm-exec` script, with the
//! lab of the `nvm exec` tests (the child is only planned).

use super::run;
use crate::commands::Output;
use crate::commands::exec::tests::{
    ACTIVE_18, BASE_PATH, N18, N20, NO_NVMRC, assert_stops, child, child_on, env_on, lab,
    npm_versions, strings,
};
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::Invocation;

const UNABLE: &str = "nvm-exec: unable to select a node version\n  Set `NODE_VERSION` \
(e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.";
const NOT_99: &str = "N/A: version \"v99\" is not yet installed.\n\nYou need to run `nvm install \
99` to install and use it.";

fn nvm_exec_in(fs: &FakeFileSystem, env: &FakeEnv, command: &[&str]) -> Output {
    let process = npm_versions();
    let context = Context::new(fs, env).with_process(&process);
    run(&context, &strings(command))
}

fn given(version: &str, command: &[&str]) -> Output {
    let env = env_on(BASE_PATH).with_var("NODE_VERSION", version);
    nvm_exec_in(&lab(), &env, command)
}

fn with_nvmrc(version: &str) -> FakeFileSystem {
    lab().with_file("/proj/.nvmrc", &format!("{version}\n"))
}

/// The child of a switch: `NODE_VERSION` is not set, it is inherited.
fn planned(path: &str, directory: &str, command: &[&str]) -> Invocation {
    let mut invocation = child_on(path, directory, "", command);
    invocation.env.retain(|(name, _)| name != "NODE_VERSION");
    invocation
}

fn assert_runs(output: &Output, stdout: &str, stderr: &str, expected: Invocation) {
    assert_eq!(output.stdout, stdout, "stdout");
    assert_eq!(output.stderr, stderr, "stderr");
    assert_eq!(output.status, NvmExitCode::Success, "status");
    assert_eq!(output.spawn, Some(expected));
}

#[test]
fn node_version_18_runs_the_command_without_printing_now_using() {
    let output = given("18", &["node", "a"]);
    assert_runs(&output, "", "", planned(BASE_PATH, N18, &["node", "a"]));
}

#[test]
fn node_version_lts_hydrogen_resolves_the_alias() {
    let output = given("lts/hydrogen", &["node", "a"]);
    assert_runs(&output, "", "", planned(BASE_PATH, N18, &["node", "a"]));
}

#[test]
fn node_version_default_puts_the_default_first_on_path() {
    let command = ["sh", "-c", "echo $PATH"];
    let output = given("default", &command);
    assert_runs(&output, "", "", planned(BASE_PATH, N18, &command));
}

#[test]
fn node_version_99_prints_the_not_installed_triple_on_stderr_and_is_127() {
    let output = given("99", &["node", "a"]);
    assert_stops(&output, "", NOT_99, NvmExitCode::NotFound);
}

#[test]
fn node_version_wins_over_an_nvmrc_and_an_active_version() {
    let env = env_on(ACTIVE_18).with_var("NODE_VERSION", "20");
    let output = nvm_exec_in(&with_nvmrc("18"), &env, &["node"]);
    assert_runs(&output, "", "", planned(ACTIVE_18, N20, &["node"]));
}

#[test]
fn a_refused_prefix_is_reported_once_and_is_127() {
    let env = env_on(BASE_PATH)
        .with_var("NODE_VERSION", "18")
        .with_var("PREFIX", "/x");
    let output = nvm_exec_in(&lab(), &env, &["node"]);
    let message = "nvm is not compatible with the \"PREFIX\" environment variable: currently \
set to \"/x\"\nRun `unset PREFIX` to unset it.";
    assert_stops(&output, "", message, NvmExitCode::NotFound);
}

#[test]
fn the_nvmrc_version_is_found_and_runs() {
    let env = env_on(BASE_PATH);
    let output = nvm_exec_in(&with_nvmrc("20"), &env, &["node", "a"]);
    let found = "Found '/proj/.nvmrc' with version <20>";
    assert_runs(&output, found, "", planned(BASE_PATH, N20, &["node", "a"]));
}

#[test]
fn an_empty_node_version_is_the_nvmrc_path() {
    let env = env_on(BASE_PATH).with_var("NODE_VERSION", "");
    let output = nvm_exec_in(&with_nvmrc("20"), &env, &["node"]);
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <20>");
    assert_eq!(output.spawn, Some(planned(BASE_PATH, N20, &["node"])));
}

#[test]
fn an_nvmrc_that_is_not_installed_prints_the_triple_and_both_lines() {
    let output = nvm_exec_in(&with_nvmrc("99"), &env_on(BASE_PATH), &["node"]);
    let stderr = format!("{NOT_99}\n{UNABLE}");
    assert_stops(
        &output,
        "Found '/proj/.nvmrc' with version <99>",
        &stderr,
        NvmExitCode::NotFound,
    );
}

#[test]
fn nothing_available_is_127_even_with_a_version_active() {
    for path in [BASE_PATH, ACTIVE_18] {
        let output = nvm_exec_in(&lab(), &env_on(path), &["node"]);
        let stderr = format!("{NO_NVMRC}\n{UNABLE}");
        assert_stops(&output, "", &stderr, NvmExitCode::NotFound);
    }
}

#[test]
fn an_invalid_nvmrc_prints_its_message_and_both_lines() {
    let output = nvm_exec_in(&with_nvmrc("foo=bar"), &env_on(BASE_PATH), &["node"]);
    assert_eq!(output.stdout, "");
    assert!(
        output.stderr.starts_with("invalid .nvmrc!\n"),
        "{}",
        output.stderr
    );
    assert!(output.stderr.ends_with(UNABLE), "{}", output.stderr);
    assert!(!output.stderr.contains("Please see"));
    assert_eq!((output.status, output.spawn), (NvmExitCode::NotFound, None));
}

#[test]
fn a_leading_double_dash_is_dropped() {
    let output = given("18", &["--", "node", "a"]);
    assert_runs(&output, "", "", planned(BASE_PATH, N18, &["node", "a"]));
}

#[test]
fn without_a_command_it_selects_and_ends_with_success() {
    let output = given("18", &[]);
    assert_eq!(output, Output::default());
}

#[test]
fn the_child_gets_the_environment_of_exec_without_node_version() {
    let output = given("18", &["node"]);
    let mut expected = child(N18, "v18.20.4", &["node"]);
    expected.env.retain(|(name, _)| name != "NODE_VERSION");
    assert_eq!(output.spawn, Some(expected));
}
