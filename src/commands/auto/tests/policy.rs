//! The automatic paths under a policy: a finding is a warning, never a stop.

use super::Lab;
use crate::error::NvmExitCode;

#[test]
fn auto_use_warns_but_still_switches() {
    let lab = Lab::new()
        .alias("default", "20")
        .policy(&["--min-node", "22"]);
    let output = lab.auto_use();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.script.is_empty());
    assert!(
        output.stderr.contains("nvm: warning: node v20.11.1"),
        "{}",
        output.stderr
    );
    assert!(
        output.stderr.contains("is not allowed by policy:"),
        "{}",
        output.stderr
    );
}

#[test]
fn auto_use_warns_about_a_pinned_tool_that_is_not_installed() {
    let lab = Lab::new().nvmrc("20\npnpm=9.12.0\n");
    let output = lab.auto_use();
    assert_eq!(output.status, NvmExitCode::Success);
    let text = "nvm: warning: pnpm 9.12.0 (from /proj/.nvmrc) is not installed. Run `nvm install`.";
    assert!(output.stderr.contains(text), "{}", output.stderr);
    assert!(!output.script.is_empty());
}

#[test]
fn auto_use_with_an_unreadable_policy_warns_and_switches() {
    let lab = Lab::new()
        .alias("default", "20")
        .var("NVMRC_POLICY", "/missing.json");
    let output = lab.auto_use();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.script.is_empty());
    assert!(
        output.stderr.starts_with("nvm: warning:"),
        "{}",
        output.stderr
    );
}

#[test]
fn auto_install_of_a_disallowed_version_warns_and_goes_on() {
    let lab = Lab::new()
        .alias("default", "18")
        .policy(&["--min-node", "22"]);
    let output = lab.auto("install").expect("__auto install runs");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output.stderr.contains("nvm: warning: node v18.20.4"),
        "{}",
        output.stderr
    );
}
