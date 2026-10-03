//! Step 12 end to end (digest 4.4): a refused prefix is status 11 with no
//! `Now using ...` line, deactivated for the variables and still switched
//! for an npmrc file; `--delete-prefix` cleans the file and goes on.

use super::{BASE_PATH, N18, env_on, lab, npm_versions, switched_to, use_in};
use crate::error::NvmExitCode;
use crate::fakes::FakeFileSystem;

const NOW_USING_18: &str = "Now using node v18.20.4 (npm v10.7.0)";
const DEACTIVATED: &str = "unset NVM_BIN\nunset NVM_INC\n";
const USER_NPMRC: &str = "Your user\u{2019}s .npmrc file (${HOME}/.npmrc)\n\
has a `globalconfig` and/or a `prefix` setting, which are incompatible with nvm.";

fn bad_user_npmrc() -> FakeFileSystem {
    lab().with_file("/h/.npmrc", "prefix=/x\n")
}

fn run_with(
    fs: &FakeFileSystem,
    variables: &[(&str, &str)],
    args: &[&str],
) -> crate::commands::Output {
    let env = variables.iter().fold(
        env_on(BASE_PATH).with_var("HOME", "/h"),
        |env, (name, value)| env.with_var(name, value),
    );
    use_in(fs, &env, &npm_versions(), args)
}

fn user_npmrc_with(command: &str) -> String {
    format!("{USER_NPMRC}\nRun `{command}` to unset it.")
}

#[test]
fn a_prefix_variable_stops_use_with_11_and_deactivates() {
    for args in [&["18"][..], &["18", "--silent"]] {
        let output = run_with(&lab(), &[("PREFIX", "/zz")], args);
        assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
        assert_eq!(output.stdout, "");
        assert_eq!(
            output.stderr,
            "nvm is not compatible with the \"PREFIX\" environment variable: currently set \
to \"/zz\"\nRun `unset PREFIX` to unset it."
        );
        assert_eq!(output.script.render(), DEACTIVATED);
    }
}

#[test]
fn a_prefix_variable_on_the_version_directory_is_accepted() {
    let output = run_with(&lab(), &[("PREFIX", N18)], &["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stdout, NOW_USING_18);
    assert_eq!(output.script.render(), switched_to(N18));
}

#[test]
fn npm_config_prefix_in_lower_case_stops_use_even_when_silent() {
    let output = run_with(
        &lab(),
        &[("npm_config_prefix", "/foo")],
        &["--silent", "18"],
    );
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(
        output.stderr,
        "nvm is not compatible with the \"npm_config_prefix\" environment variable: \
currently set to \"/foo\"\nRun `unset npm_config_prefix` to unset it."
    );
    assert_eq!(output.script.render(), DEACTIVATED);
}

#[test]
fn deactivating_takes_an_active_version_out_of_path() {
    let path = format!("/n/versions/node/v20.11.1/bin:{BASE_PATH}");
    let env = env_on(&path).with_var("NPM_CONFIG_PREFIX", "/foo");
    let output = use_in(&lab(), &env, &npm_versions(), &["18"]);
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(
        output.script.render(),
        format!("export PATH='{BASE_PATH}'\nhash -r 2>/dev/null || true\n{DEACTIVATED}")
    );
}

#[test]
fn npm_config_prefix_inside_nvm_dir_is_accepted() {
    let output = run_with(&lab(), &[("NPM_CONFIG_PREFIX", N18)], &["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stdout, NOW_USING_18);
}

#[test]
fn a_bad_npmrc_stops_use_with_11_and_keeps_the_switch() {
    let output = run_with(&bad_user_npmrc(), &[], &["18"]);
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(output.stdout, "");
    assert_eq!(
        output.stderr,
        user_npmrc_with("nvm use --delete-prefix v18.20.4")
    );
    assert_eq!(output.script.render(), switched_to(N18));
}

#[test]
fn silent_does_not_hide_the_npmrc_message_and_joins_the_command() {
    let output = run_with(&bad_user_npmrc(), &[], &["18", "--silent"]);
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(
        output.stderr,
        user_npmrc_with("nvm use --delete-prefix v18.20.4 --silent")
    );
}

#[test]
fn lts_alone_names_no_version_in_the_command() {
    let output = run_with(&bad_user_npmrc(), &[], &["--lts"]);
    assert_eq!(output.stderr, user_npmrc_with("nvm use --delete-prefix"));
    assert_eq!(
        output.script.render(),
        switched_to("/n/versions/node/v20.11.1")
    );
}

#[test]
fn a_version_from_nvmrc_is_named_after_the_found_line() {
    let fs = bad_user_npmrc().with_file("/proj/.nvmrc", "18\n");
    let output = run_with(&fs, &[], &[]);
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <18>");
    assert_eq!(
        output.stderr,
        user_npmrc_with("nvm use --delete-prefix v18.20.4")
    );
}

#[test]
fn delete_prefix_runs_npm_config_then_says_now_using() {
    let npm = format!("{N18}/bin/npm");
    let process = npm_versions()
        .with_success(
            &npm,
            "config --loglevel=warn delete prefix --userconfig=/h/.npmrc",
            "",
        )
        .with_success(
            &npm,
            "config --loglevel=warn delete globalconfig --userconfig=/h/.npmrc",
            "",
        );
    let env = env_on(BASE_PATH).with_var("HOME", "/h");
    let output = use_in(
        &bad_user_npmrc(),
        &env,
        &process,
        &["--delete-prefix", "18"],
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        (NOW_USING_18, "")
    );
    assert_eq!(output.script.render(), switched_to(N18));
    let calls: Vec<String> = process
        .executed()
        .iter()
        .map(|run| run.args.join(" "))
        .collect();
    assert_eq!(
        calls,
        [
            "--version",
            "config --loglevel=warn delete prefix --userconfig=/h/.npmrc",
            "config --loglevel=warn delete globalconfig --userconfig=/h/.npmrc",
        ]
    );
}

#[test]
fn use_system_skips_the_prefix_checks() {
    let fs = bad_user_npmrc().with_executable("/usr/bin/node", "");
    let output = run_with(&fs, &[("PREFIX", "/zz")], &["system", "--silent"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stderr, "");
}
