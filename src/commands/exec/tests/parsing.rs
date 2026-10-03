//! Digest 6.1: the options of `nvm exec`, only before the version.

use super::*;

const RUNNING_18: &str = "Running node v18.20.4 (npm v10.7.0)";

#[test]
fn an_unknown_long_option_is_unsupported_with_55() {
    let output = exec(&["--bogus", "18", "node"]);
    let message = "Unsupported option \"--bogus\".";
    assert_stops(&output, "", message, NvmExitCode::UnsupportedOption);
}

#[test]
fn empty_arguments_before_the_version_are_skipped() {
    let output = exec(&["", "18", "node", "a"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &["node", "a"]));
}

#[test]
fn options_after_the_version_go_to_the_command() {
    let output = exec(&["18", "node", "--silent", "--lts", "--bogus"]);
    let command = ["node", "--silent", "--lts", "--bogus"];
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &command));
}

#[test]
fn a_double_dash_after_the_version_is_dropped_before_the_command() {
    let output = exec(&["18", "--", "node", "a"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &["node", "a"]));
}

#[test]
fn a_double_dash_first_stops_the_options_and_is_taken_for_the_command() {
    let output = exec(&["--", "18", "node", "a"]);
    let stderr = format!("{NO_NVMRC}\n{WARNING}\n{CURRENT_NOT_INSTALLED}");
    assert_stops(&output, "", &stderr, NvmExitCode::Failure);
}

#[test]
fn a_double_dash_first_with_a_version_active_runs_the_word_after_it() {
    let output = exec_in(&lab(), &env_on(ACTIVE_18), &["--", "18", "node"]);
    assert_eq!(output.stdout, RUNNING_18);
    let expected = child_on(ACTIVE_18, N18, "v18.20.4", &["18", "node"]);
    assert_eq!(output.spawn, Some(expected));
}

#[test]
fn an_empty_lts_name_is_no_lts() {
    let output = exec(&["--lts=", "18", "node"]);
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &["node"]));
}

#[test]
fn the_help_words_are_passed_to_the_command() {
    // DELIBERATE DEVIATION: nvm.sh prints its help for any `-h`, `help` or
    // `--help` before a `--`; here they reach the command.
    let output = exec(&["18", "node", "-h", "help", "--help"]);
    let command = ["node", "-h", "help", "--help"];
    assert_runs(&output, RUNNING_18, child(N18, "v18.20.4", &command));
}
