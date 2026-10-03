//! `nvm __auto install` (digest 10): the raw target of the `default` alias,
//! else the `.nvmrc`, installed with the stdout of `install` dropped.

use super::{Lab, assert_unchanged};
use crate::commands::Output;
use crate::error::NvmExitCode;

fn auto_install(lab: &Lab) -> Output {
    lab.auto("install").expect("__auto install runs")
}

#[test]
fn a_valid_default_target_is_installed_quietly() {
    let output = auto_install(&Lab::new().alias("default", "18"));
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "v18.20.4 is already installed.");
}

#[test]
fn without_a_valid_default_target_or_an_nvmrc_nothing_is_installed() {
    assert_unchanged(&auto_install(&Lab::new()));
    for target in ["lts/*", "garbage!", "system"] {
        assert_unchanged(&auto_install(&Lab::new().alias("default", target)));
    }
}

#[test]
fn a_default_that_is_not_a_version_falls_back_to_the_nvmrc() {
    let lab = Lab::new().alias("default", "lts/*").nvmrc("18\n");
    let output = auto_install(&lab);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "v18.20.4 is already installed.");
}

#[test]
fn in_the_nvm_function_the_installed_version_is_activated() {
    let lab = Lab::new().alias("default", "18").in_function();
    let output = auto_install(&lab);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "v18.20.4 is already installed.");
    let script = output.script.render();
    assert!(
        script.contains("export PATH='/n/versions/node/v18.20.4/bin:"),
        "{script}"
    );
}

#[test]
fn an_invalid_nvmrc_installs_nothing() {
    assert_unchanged(&auto_install(&Lab::new().nvmrc("18\n20\n")));
}
