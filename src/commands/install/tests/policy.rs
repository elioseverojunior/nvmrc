//! `nvm install` under a policy: the gate runs after the version is known
//! and before anything is downloaded.

use super::*;
use crate::domain::policy::{DEFAULT_INDEX, extract};
use std::ffi::OsString;

fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

#[test]
fn install_below_a_policy_minimum_stops_before_downloading() {
    let mut world = World::new();
    world.policy_flags = flags(&["--min-node", "22"]);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        lines(&output.stderr),
        [
            "nvm: node v20.10.0 (from the command line) is not allowed by policy:",
            "  - below the minimum v22.0.0 (from --min-node)",
            "Use an allowed version.",
        ]
    );
    assert!(!world.installed());
    assert!(!world.http.requests().contains(&TARBALL_URL.to_owned()));
}

#[test]
fn a_version_from_nvmrc_names_the_file_in_the_message() {
    let mut world = World::new().with_env(&[("PWD", "/p/sub")]);
    world.fs = FakeFileSystem::default().with_file("/p/.nvmrc", "20\n");
    world.policy_flags = flags(&["--min-node", "22"]);
    let output = world.run("").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(
        output
            .stderr
            .contains("nvm: node v20.10.0 (from /p/.nvmrc) is not allowed by policy:")
    );
    assert!(
        output
            .stderr
            .ends_with("Raise /p/.nvmrc to an allowed version.")
    );
}

#[test]
fn install_of_a_vulnerable_version_stops_before_downloading() {
    let index = r#"{"1": {"cve": ["CVE-2024-27983"], "vulnerable": "20.x",
                         "patched": "^20.12.1", "severity": "high"}}"#;
    let mut world = World::new();
    world.http = std::mem::take(&mut world.http).with_body(DEFAULT_INDEX, index);
    world.policy_flags = flags(&["--fail-on-severity", "high"]);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        lines(&output.stderr),
        [
            "nvm: node v20.10.0 (from the command line) is not allowed by policy:",
            "  - CVE-2024-27983 (high); the limit is high (from --fail-on-severity)",
            "Use an allowed version.",
        ]
    );
    assert!(!world.installed());
    assert!(!world.http.requests().contains(&TARBALL_URL.to_owned()));
}

#[test]
fn install_in_legacy_mode_keeps_todays_floor_message() {
    let world = World::new().with_env(&[("NVM_MIN_VERSION", "v22")]);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        lines(&output.stderr),
        [
            "Version v20.10.0 is below the minimum allowed version v22.0.0.",
            "Lower or unset NVM_MIN_VERSION (or edit /n/min-version) to install it.",
        ]
    );
}
