//! Digest 6.4: one test per observed row of `nvm exec` (the exit status of
//! the child itself is the CLI's: here the child is only planned).

use super::*;

const RUNNING_18: &str = "Running node v18.20.4 (npm v10.7.0)";
const N22: &str = "/n/versions/node/v22.3.0";

fn with_nvmrc(version: &str) -> FakeFileSystem {
    lab().with_file("/proj/.nvmrc", &format!("{version}\n"))
}

fn nothing_to_run() -> String {
    format!("{NO_NVMRC}\n{WARNING}\n{CURRENT_NOT_INSTALLED}")
}

#[test]
fn exec_18_node_version_names_the_version_and_its_npm() {
    let output = exec(&["18", "node", "--version"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &["node", "--version"]),
    );
}

#[test]
fn exec_18_node_exit_7_runs_the_command_whose_status_is_passed_on() {
    let output = exec(&["18", "node", "exit", "7"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &["node", "exit", "7"]),
    );
}

#[test]
fn exec_silent_prints_nothing_before_the_command() {
    let output = exec(&["--silent", "18", "node", "exit", "4"]);
    assert_runs(&output, "", child(N18, "v18.20.4", &["node", "exit", "4"]));
}

#[test]
fn exec_lts_names_the_latest_lts_and_passes_it_unresolved() {
    let output = exec(&["--lts", "node", "a"]);
    let stdout = "Running node latest LTS -> v20.11.1 (npm v10.2.4)";
    assert_runs(&output, stdout, child(N20, "lts/*", &["node", "a"]));
}

#[test]
fn exec_lts_hydrogen_names_the_lts_line() {
    let output = exec(&["--lts=hydrogen", "node", "a"]);
    let stdout = "Running node LTS \"hydrogen\" -> v18.20.4 (npm v10.7.0)";
    assert_runs(&output, stdout, child(N18, "lts/hydrogen", &["node", "a"]));
}

#[test]
fn exec_lts_argon_is_not_installed() {
    let output = exec(&["--lts=argon", "node", "a"]);
    let stderr = "N/A: version \"lts/argon\" is not yet installed.\n\n\
You need to run `nvm install lts/argon` to install and use it.";
    assert_stops(&output, "", stderr, NvmExitCode::Failure);
}

#[test]
fn exec_an_lts_alias_as_the_version_resolves_it() {
    let output = exec(&["lts/hydrogen", "node", "a"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &["node", "a"]));
}

#[test]
fn exec_an_unknown_lts_alias_is_taken_for_the_command() {
    let output = exec(&["lts/argon", "node", "a"]);
    assert_stops(&output, "", &nothing_to_run(), NvmExitCode::Failure);
}

#[test]
fn exec_99_is_a_version_that_is_not_installed() {
    let output = exec(&["99", "node", "a"]);
    let stderr = "N/A: version \"v99\" is not yet installed.\n\n\
You need to run `nvm install 99` to install and use it.";
    assert_stops(&output, "", stderr, NvmExitCode::Failure);
}

#[test]
fn exec_an_alias_to_nothing_installed_is_taken_for_the_command() {
    let output = exec(&["missing", "node", "a"]);
    assert_stops(&output, "", &nothing_to_run(), NvmExitCode::Failure);
}

#[test]
fn exec_a_looping_alias_is_not_installed() {
    let output = exec(&["loopa", "node", "a"]);
    let stderr = "N/A: version \"loopa -> ∞\" is not yet installed.\n\n\
You need to run `nvm install loopa` to install and use it.";
    assert_stops(&output, "", stderr, NvmExitCode::Failure);
}

#[test]
fn exec_node_runs_the_newest_node_without_an_npm_suffix() {
    let output = exec(&["node", "node", "a"]);
    assert_runs(
        &output,
        "Running node v22.3.0",
        child(N22, "v22.3.0", &["node", "a"]),
    );
}

#[test]
fn exec_22_runs_v22() {
    let output = exec(&["22", "node", "a"]);
    assert_runs(
        &output,
        "Running node v22.3.0",
        child(N22, "v22.3.0", &["node", "a"]),
    );
}

#[test]
fn exec_iojs_names_io_js_without_its_prefix() {
    let output = exec(&["iojs", "iojs", "a"]);
    let directory = "/n/versions/io.js/v3.3.1";
    let stdout = "Running io.js v3.3.1 (npm v2.14.3)";
    assert_runs(
        &output,
        stdout,
        child(directory, "iojs-v3.3.1", &["iojs", "a"]),
    );
}

#[test]
fn exec_0_10_runs_the_old_layout() {
    let output = exec(&["0.10", "node", "a"]);
    let stdout = "Running node v0.10.48 (npm v2.15.1)";
    assert_runs(
        &output,
        stdout,
        child("/n/v0.10.48", "v0.10.48", &["node", "a"]),
    );
}

#[test]
fn exec_a_command_with_nothing_active_and_no_nvmrc_fails() {
    let output = exec(&["npm", "-v"]);
    assert_stops(&output, "", &nothing_to_run(), NvmExitCode::Failure);
}

#[test]
fn exec_a_command_with_18_active_warns_and_runs_on_it() {
    let output = exec_in(&lab(), &env_on(ACTIVE_18), &["npm", "-v"]);
    assert_streams(
        &output,
        RUNNING_18,
        &format!("{NO_NVMRC}\n{WARNING}"),
        NvmExitCode::Success,
    );
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &["npm", "-v"]);
    assert_eq!(output.spawn, Some(expected));
}

#[test]
fn exec_current_with_18_active_runs_on_it_without_a_warning() {
    let output = exec_in(&lab(), &env_on(ACTIVE_18), &["current", "node", "a"]);
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &["node", "a"]);
    assert_runs(&output, RUNNING_18, expected);
}

#[test]
fn exec_a_version_without_a_command_only_says_what_it_would_run() {
    let output = exec(&["18"]);
    assert_stops(&output, RUNNING_18, "", NvmExitCode::Success);
}

#[test]
fn exec_without_arguments_warns_without_reading_the_nvmrc() {
    let stderr = format!("{WARNING}\n{CURRENT_NOT_INSTALLED}");
    for fs in [lab(), with_nvmrc("20")] {
        let output = exec_in(&fs, &env_on(BASE_PATH), &[]);
        assert_stops(&output, "", &stderr, NvmExitCode::Failure);
    }
}

#[test]
fn exec_a_command_that_does_not_exist_still_runs_it() {
    let output = exec(&["18", "nonexistentcmd"]);
    assert_runs(
        &output,
        RUNNING_18,
        child(N18, "v18.20.4", &["nonexistentcmd"]),
    );
}

#[test]
fn exec_a_command_with_an_nvmrc_runs_on_its_version() {
    let output = exec_in(&with_nvmrc("20"), &env_on(BASE_PATH), &["npm", "-v"]);
    let stdout = "Found '/proj/.nvmrc' with version <20>\nRunning node v20.11.1 (npm v10.2.4)";
    assert_runs(&output, stdout, child(N20, "v20.11.1", &["npm", "-v"]));
}

#[test]
fn exec_silent_with_an_nvmrc_prints_nothing() {
    let output = exec_in(
        &with_nvmrc("20"),
        &env_on(BASE_PATH),
        &["--silent", "npm", "-v"],
    );
    assert_runs(&output, "", child(N20, "v20.11.1", &["npm", "-v"]));
}

#[test]
fn exec_node_with_an_nvmrc_takes_node_for_the_version() {
    let output = exec_in(&with_nvmrc("20"), &env_on(BASE_PATH), &["node", "a"]);
    assert_runs(
        &output,
        "Running node v22.3.0",
        child(N22, "v22.3.0", &["a"]),
    );
}

#[test]
fn exec_with_an_nvmrc_naming_a_missing_version_fails_with_the_plain_hint() {
    let output = exec_in(&with_nvmrc("99"), &env_on(BASE_PATH), &["npm", "-v"]);
    let stderr = "N/A: version \"v99\" is not yet installed.\n\n\
You need to run `nvm install 99` to install and use it.";
    let stdout = "Found '/proj/.nvmrc' with version <99>";
    assert_stops(&output, stdout, stderr, NvmExitCode::Failure);
}
