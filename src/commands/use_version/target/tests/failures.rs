//! The rows of digest 4.3 that end in an error: exact stderr and status.

use super::*;

const PLEASE_SEE: &str =
    "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.";

fn not_installed(shown: &str, provided: &str) -> String {
    format!(
        "N/A: version \"{shown}\" is not yet installed.\n\n\
         You need to run `nvm install {provided}` to install and use it."
    )
}

fn assert_fails_in(
    fs: &FakeFileSystem,
    env: &FakeEnv,
    args: &[&str],
    status: NvmExitCode,
    stderr: &str,
) {
    let (target, output) = run_in(fs, env, args);
    assert_eq!(target, Err(Halt { status }), "{args:?}");
    assert_eq!(output.stdout, "", "{args:?}");
    assert_eq!(output.stderr, stderr, "{args:?}");
}

fn assert_fails(args: &[&str], status: NvmExitCode, stderr: &str) {
    assert_fails_in(&lab(), &env_on(BASE_PATH), args, status, stderr);
}

#[test]
fn an_lts_that_is_not_installed_names_the_empty_provided_version() {
    let message = not_installed("", "");
    assert_fails(&["--lts=argon"], NvmExitCode::InvalidVersion, &message);
    let no_lts = lab().with_file("/n/alias/lts/*", "");
    assert_fails_in(
        &no_lts,
        &env_on(BASE_PATH),
        &["--lts"],
        NvmExitCode::InvalidVersion,
        &message,
    );
    assert_fails(
        &["--lts=argon", "--silent"],
        NvmExitCode::InvalidVersion,
        "",
    );
}

#[test]
fn an_lts_miss_says_nothing_when_what_was_provided_is_installed() {
    assert_fails(&["--lts=argon", "18"], NvmExitCode::InvalidVersion, "");
    let active = env_on("/n/versions/node/v18.20.4/bin:/usr/bin");
    assert_fails_in(
        &lab(),
        &active,
        &["--lts=argon"],
        NvmExitCode::InvalidVersion,
        "",
    );
}

#[test]
fn versions_that_are_not_installed_are_status_3() {
    let rows = [
        ("lts/argon", "lts/argon"),
        ("99", "v99"),
        ("18.20.4.5", "v18.20.4.5"),
        ("1", "v1"),
        ("missing", "missing -> v99"),
        ("foo", "foo"),
        ("-", "-"),
        ("unstable", "unstable"),
    ];
    for (provided, shown) in rows {
        let message = not_installed(shown, provided);
        assert_fails(&[provided], NvmExitCode::InvalidVersion, &message);
    }
}

#[test]
fn lts_alone_is_not_an_alias() {
    let message = "N/A: version \"lts\" is not yet installed.\n\n\
        `lts` is not an alias - you may need to run `nvm install --lts` to install and `nvm use --lts` to use it.";
    assert_fails(&["lts"], NvmExitCode::InvalidVersion, message);
}

#[test]
fn silent_hides_the_not_installed_message() {
    assert_fails(&["--silent", "99"], NvmExitCode::InvalidVersion, "");
}

#[test]
fn an_alias_loop_is_status_8() {
    let message = "The alias \"loopa\" leads to an infinite loop. Aborting.";
    assert_fails(&["loopa"], NvmExitCode::AliasLoop, message);
    assert_fails(&["--silent", "loopa"], NvmExitCode::AliasLoop, "");
}

#[test]
fn an_lts_alias_loop_names_the_empty_provided_version() {
    let fs = lab().with_file("/n/alias/lts/x", "loopa");
    let message = "The alias \"\" leads to an infinite loop. Aborting.";
    assert_fails_in(
        &fs,
        &env_on(BASE_PATH),
        &["--lts=x"],
        NvmExitCode::AliasLoop,
        message,
    );
    let named = "The alias \"18\" leads to an infinite loop. Aborting.";
    assert_fails_in(
        &fs,
        &env_on(BASE_PATH),
        &["--lts=x", "18"],
        NvmExitCode::AliasLoop,
        named,
    );
}

/// DELIBERATE DEVIATION: nvm.sh returns 0 here (its step 9 returns the status
/// of a `!` test); the message is the same, the status is 3.
#[test]
fn current_with_nothing_active_is_status_3_with_the_message_even_when_silent() {
    let message = not_installed("none", "none");
    assert_fails(&["current"], NvmExitCode::InvalidVersion, &message);
    assert_fails(
        &["current", "--silent"],
        NvmExitCode::InvalidVersion,
        &message,
    );
}

/// The same step 9: a version directory without its `bin/node`.
#[test]
fn a_version_directory_without_its_node_is_status_3_naming_the_version() {
    let fs = lab().with_dir("/n/versions/node/v24.0.0");
    let message = not_installed("v24.0.0", "v24.0.0");
    assert_fails_in(
        &fs,
        &env_on(BASE_PATH),
        &["24"],
        NvmExitCode::InvalidVersion,
        &message,
    );
}

#[test]
fn system_without_a_system_node_is_status_127() {
    let message = "System version of node not found.";
    assert_fails(&["system"], NvmExitCode::NotFound, message);
    assert_fails(&["system", "--silent"], NvmExitCode::NotFound, "");
}

#[test]
fn a_node_inside_nvm_dir_is_not_a_system_node() {
    let env = env_on("/n/versions/node/v18.20.4/bin:/usr/bin");
    let message = "System version of node not found.";
    assert_fails_in(&lab(), &env, &["system"], NvmExitCode::NotFound, message);
}

#[test]
fn no_version_and_no_nvmrc_is_status_127() {
    let message = format!("No version provided and no .nvmrc file found\n{PLEASE_SEE}");
    for args in [&[][..], &[""], &["--lts="]] {
        assert_fails(args, NvmExitCode::NotFound, &message);
    }
    assert_fails(&["--silent"], NvmExitCode::NotFound, PLEASE_SEE);
}

#[test]
fn save_given_twice_is_status_6_before_anything_else() {
    let message = "--save and -w may only be provided once";
    assert_fails(
        &["--save", "-w", "18"],
        NvmExitCode::InvalidOptions,
        message,
    );
}

#[test]
fn an_unknown_nvm_dir_is_reported() {
    let (fs, env) = (lab(), FakeEnv::default().with_var("PATH", BASE_PATH));
    let message = "Neither NVM_DIR nor HOME is set; cannot locate the nvm directory.";
    assert_fails_in(&fs, &env, &["18"], NvmExitCode::Failure, message);
}
