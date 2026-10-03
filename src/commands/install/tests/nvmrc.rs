//! `nvm install` with no version reads `.nvmrc` (digest 9): every row of the
//! observed table, in the `nvm` function (the shell-code channel open), with
//! v18.19.0 and v20.10.0 installed and `default -> 18` unless said otherwise.

use super::*;
use crate::domain::nvmrc::invalid_message;

pub(super) const V18_NODE: &str = "/n/versions/node/v18.19.0/bin/node";
pub(super) const SHELL: [(&str, &str); 3] = [
    ("PWD", "/p/sub"),
    ("PATH", "/usr/bin"),
    ("NVMRC_SCRIPT_FD", "3"),
];
pub(super) const USAGE: &str = "Usage: nvm install [<version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";
const FOUND_18: &str = "Found '/p/.nvmrc' with version <18>";
const FOUND_20: &str = "Found '/p/.nvmrc' with version <20>";
const FOUND_LTS: &str = "Found '/p/.nvmrc' with version <lts/*>";
const FOUND_HYDROGEN: &str = "Found '/p/.nvmrc' with version <lts/hydrogen>";
const FOUND_NODE: &str = "Found '/p/.nvmrc' with version <node>";
const FOUND_99: &str = "Found '/p/.nvmrc' with version <99>";
const NO_NVMRC: &str = "No version provided and no .nvmrc file found";

fn both_installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable(V18_NODE, "x")
        .with_executable(NODE, "x")
}

/// Both versions installed, `default -> 18`, and `.nvmrc` holding `nvmrc`.
fn lab(nvmrc: Option<&str>) -> World {
    let fs = both_installed().with_file("/n/alias/default", "18\n");
    let fs = match nvmrc {
        Some(text) => fs.with_file("/p/.nvmrc", text),
        None => fs,
    };
    World { fs, ..World::new() }.with_env(&SHELL)
}

fn assert_run(output: &Output, stdout: &[&str], stderr: &[&str], status: NvmExitCode) {
    assert_eq!(lines(&output.stdout), stdout, "stdout");
    assert_eq!(lines(&output.stderr), stderr, "stderr");
    assert_eq!(output.status, status);
}

#[test]
fn no_nvmrc_is_the_lookup_message_then_the_usage_with_status_127() {
    let output = lab(None).run("").unwrap();
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, format!("{NO_NVMRC}\n{USAGE}"));
    assert_eq!(output.status, NvmExitCode::NotFound);
    assert!(output.script.is_empty());
}

#[test]
fn lts_without_an_nvmrc_uses_the_newest_lts() {
    let output = lab(None).run("--lts").unwrap();
    let stdout = ["Installing latest LTS version.", "Now using node v20.10.0"];
    assert_run(
        &output,
        &stdout,
        &["v20.10.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn a_named_lts_without_an_nvmrc_uses_that_line() {
    let output = lab(None).run("--lts=hydrogen").unwrap();
    let stdout = [
        "Installing with latest version of LTS line: hydrogen",
        "Now using node v18.19.0",
    ];
    assert_run(
        &output,
        &stdout,
        &["v18.19.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn an_unknown_lts_line_is_status_3() {
    let output = lab(None).run("--lts=argon").unwrap();
    let stderr = "Version with LTS filter 'argon' not found - try `nvm ls-remote --lts=argon` to browse available versions.";
    let stdout = ["Installing with latest version of LTS line: argon"];
    assert_run(&output, &stdout, &[stderr], NvmExitCode::InvalidVersion);
}

#[test]
fn an_uppercase_lts_line_ignores_the_nvmrc_and_is_status_3() {
    let output = lab(Some("18\n")).run("--lts=Hydrogen").unwrap();
    let stderr = [
        "LTS names must be lowercase",
        "Version with LTS filter 'Hydrogen' not found - try `nvm ls-remote --lts=Hydrogen` to browse available versions.",
    ];
    let stdout = ["Installing with latest version of LTS line: Hydrogen"];
    assert_run(&output, &stdout, &stderr, NvmExitCode::InvalidVersion);
}

#[test]
fn the_nvmrc_version_is_found_and_used() {
    let output = lab(Some("18\n")).run("").unwrap();
    let stdout = [FOUND_18, "Now using node v18.19.0"];
    assert_run(
        &output,
        &stdout,
        &["v18.19.0 is already installed."],
        NvmExitCode::Success,
    );
    assert!(
        output
            .script
            .render()
            .contains("/n/versions/node/v18.19.0/bin")
    );
}

#[test]
fn lts_ignores_the_nvmrc() {
    let output = lab(Some("18\n")).run("--lts").unwrap();
    let stdout = ["Installing latest LTS version.", "Now using node v20.10.0"];
    assert_run(
        &output,
        &stdout,
        &["v20.10.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn lts_star_in_the_nvmrc_is_the_newest_lts() {
    let output = lab(Some("lts/*\n")).run("").unwrap();
    let stdout = [FOUND_LTS, "Now using node v20.10.0"];
    assert_run(
        &output,
        &stdout,
        &["v20.10.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn a_named_lts_in_the_nvmrc_is_that_line() {
    let output = lab(Some("lts/hydrogen\n")).run("").unwrap();
    let stdout = [FOUND_HYDROGEN, "Now using node v18.19.0"];
    assert_run(
        &output,
        &stdout,
        &["v18.19.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn node_in_the_nvmrc_is_the_newest_version() {
    let output = lab(Some("node\n")).run("").unwrap();
    let stdout = [FOUND_NODE, "Now using node v20.10.0"];
    assert_run(
        &output,
        &stdout,
        &["v20.10.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn an_nvmrc_version_the_mirror_lacks_is_status_3() {
    let output = lab(Some("99\n")).run("").unwrap();
    let stderr = "Version '99' not found - try `nvm ls-remote` to browse available versions.";
    assert_run(&output, &[FOUND_99], &[stderr], NvmExitCode::InvalidVersion);
}

#[test]
fn an_invalid_nvmrc_is_its_message_then_the_usage_with_status_127() {
    let output = lab(Some("18\n20\n")).run("").unwrap();
    let parsed = ["18".to_owned(), "20".to_owned()];
    let message = invalid_message(&parsed);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, format!("{}\n{USAGE}", message.trim_end()));
    assert_eq!(output.status, NvmExitCode::NotFound);
}

#[test]
fn without_a_default_alias_the_raw_nvmrc_text_becomes_the_default() {
    let world = World {
        fs: both_installed().with_file("/p/.nvmrc", "18\n"),
        ..World::new()
    }
    .with_env(&SHELL);
    let output = world.run("").unwrap();
    let stdout = [
        FOUND_18,
        "Now using node v18.19.0",
        "Creating default alias: default -> 18 (-> v18.19.0 *)",
    ];
    assert_run(
        &output,
        &stdout,
        &["v18.19.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn save_writes_the_resolved_version_to_the_current_directory() {
    let world = lab(Some("18\n"));
    let output = world.run("--save").unwrap();
    let stdout = [
        FOUND_18,
        "Now using node v18.19.0",
        "Wrote version number (v18.19.0) to .nvmrc",
    ];
    assert_run(
        &output,
        &stdout,
        &["v18.19.0 is already installed."],
        NvmExitCode::Success,
    );
    assert_eq!(world.text("/p/sub/.nvmrc").as_deref(), Some("v18.19.0\n"));
}

#[test]
fn the_version_in_use_is_used_again() {
    let world = lab(Some("20\n")).with_env(&[
        ("PWD", "/p/sub"),
        ("PATH", "/n/versions/node/v20.10.0/bin:/usr/bin"),
        ("NVMRC_SCRIPT_FD", "3"),
    ]);
    let output = world.run("").unwrap();
    let stdout = [FOUND_20, "Now using node v20.10.0"];
    assert_run(
        &output,
        &stdout,
        &["v20.10.0 is already installed."],
        NvmExitCode::Success,
    );
}

#[test]
fn offline_and_binary_only_read_the_nvmrc_the_same_way() {
    for line in ["--offline", "-b"] {
        let output = lab(Some("18\n")).run(line).unwrap();
        let stdout = [FOUND_18, "Now using node v18.19.0"];
        assert_run(
            &output,
            &stdout,
            &["v18.19.0 is already installed."],
            NvmExitCode::Success,
        );
    }
}

#[test]
fn an_empty_version_argument_reads_the_nvmrc_too() {
    let world = lab(Some("18\n"));
    let output = world.run_words(&[String::new()]).unwrap();
    assert_eq!(lines(&output.stdout), [FOUND_18, "Now using node v18.19.0"]);
}

#[test]
fn an_empty_version_argument_without_an_nvmrc_is_not_the_usage() {
    let output = lab(None).run_words(&[String::new()]).unwrap();
    let stderr = lines(&output.stderr);
    assert_eq!(stderr.first(), Some(&NO_NVMRC));
    assert!(!output.stderr.contains("Usage:"));
    assert_eq!(output.status, NvmExitCode::Success);
}
