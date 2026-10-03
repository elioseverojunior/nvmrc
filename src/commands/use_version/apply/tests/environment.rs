//! The parts of the switch that depend on the environment: `MANPATH` when a
//! `manpath` exists, the `NVM_SYMLINK_CURRENT` link, and the failures that
//! stop before any change.

use std::path::{Path, PathBuf};

use super::{BASE_PATH, HASH, N18, env_on, lab, npm_versions, switched_to, use_in};
use crate::commands::use_version::run;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use crate::ports::FileSystem;

const NOW_USING_18: &str = "Now using node v18.20.4 (npm v10.7.0)";

fn with_manpath(manpath: Option<&str>) -> String {
    let fs = lab().with_executable("/usr/bin/manpath", "");
    let env = env_on(BASE_PATH);
    let env = match manpath {
        Some(value) => env.with_var("MANPATH", value),
        None => env,
    };
    let output = use_in(&fs, &env, &npm_versions(), &["18"]);
    assert_eq!(output.stdout, NOW_USING_18);
    output.script.render()
}

fn manpath_then_switch(manpath: &str) -> String {
    format!("export MANPATH='{manpath}'\n{}", switched_to(N18))
}

#[test]
fn with_manpath_an_unset_manpath_gets_the_version_and_a_trailing_colon() {
    let expected = manpath_then_switch(&format!("{N18}/share/man:"));
    assert_eq!(with_manpath(None), expected);
}

#[test]
fn with_manpath_the_entry_is_prepended_or_replaced() {
    let prepended = manpath_then_switch(&format!("{N18}/share/man:/m1:/m2:"));
    assert_eq!(with_manpath(Some("/m1:/m2")), prepended);
    let active = "/m1:/n/versions/node/v20.11.1/share/man:/m2";
    let replaced = manpath_then_switch(&format!("/m1:{N18}/share/man:/m2:"));
    assert_eq!(with_manpath(Some(active)), replaced);
    let empty_entry = manpath_then_switch(&format!("{N18}/share/man::/m1"));
    assert_eq!(with_manpath(Some(":/m1")), empty_entry);
}

#[test]
fn without_manpath_the_manpath_is_left_alone() {
    let env = env_on(BASE_PATH).with_var("MANPATH", "/m1");
    let output = use_in(&lab(), &env, &npm_versions(), &["18"]);
    assert_eq!(output.script.render(), switched_to(N18));
}

#[test]
fn an_nvm_entry_after_usr_bin_is_kept_and_the_version_prepended() {
    let path = "/usr/bin:/bin:/n/versions/node/v20.11.1/bin:/b";
    let output = use_in(&lab(), &env_on(path), &npm_versions(), &["18"]);
    let expected = format!("export PATH='{N18}/bin:{path}'\n{HASH}");
    assert!(output.script.render().starts_with(&expected));
}

fn link_after(symlink_current: &str) -> std::io::Result<PathBuf> {
    let fs = lab();
    fs.symlink(
        Path::new("/n/versions/node/v20.11.1"),
        Path::new("/n/current"),
    )
    .expect("link");
    let env = env_on(BASE_PATH).with_var("NVM_SYMLINK_CURRENT", symlink_current);
    let output = use_in(&fs, &env, &npm_versions(), &["18"]);
    assert_eq!(output.stdout, NOW_USING_18);
    assert_eq!(output.stderr, "");
    fs.read_link(Path::new("/n/current"))
}

#[test]
fn symlink_current_true_points_current_at_the_version() {
    assert_eq!(link_after("true").expect("linked"), PathBuf::from(N18));
    let fs = lab();
    let env = env_on(BASE_PATH).with_var("NVM_SYMLINK_CURRENT", "true");
    use_in(&fs, &env, &npm_versions(), &["18"]);
    assert_eq!(
        fs.read_link(Path::new("/n/current")).expect("new"),
        PathBuf::from(N18)
    );
}

#[test]
fn symlink_current_other_than_true_leaves_current_alone() {
    let unchanged = PathBuf::from("/n/versions/node/v20.11.1");
    assert_eq!(link_after("1").expect("kept"), unchanged);
}

#[test]
fn a_version_that_is_not_installed_halts_with_its_messages_and_no_code() {
    let output = use_in(&lab(), &env_on(BASE_PATH), &npm_versions(), &["99"]);
    assert_eq!(output.stdout, "");
    let expected = "N/A: version \"v99\" is not yet installed.\n\n\
You need to run `nvm install 99` to install and use it.";
    assert_eq!(output.stderr, expected);
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert!(output.script.is_empty());
}

#[test]
fn a_silent_failure_prints_nothing() {
    let output = use_in(
        &lab(),
        &env_on(BASE_PATH),
        &npm_versions(),
        &["--silent", "99"],
    );
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert!(output.script.is_empty());
}

#[test]
fn save_given_twice_is_an_error_with_status_6() {
    let fs = lab();
    let env = env_on(BASE_PATH);
    let args = ["--save".to_owned(), "-w".to_owned(), "18".to_owned()];
    let error = run(&Context::new(&fs, &env), &args).expect_err("rejected");
    assert!(matches!(error, CliError::InvalidOptions(_)));
    assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions);
}
