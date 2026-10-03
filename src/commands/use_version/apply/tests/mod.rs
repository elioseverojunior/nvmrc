//! `nvm use` end to end with fakes: the shell code and the stdout line of
//! every successful row of the oracle (digest 4.3), against the lab of the
//! digest with an `npm` next to each `node` but v22.3.0's.

mod environment;
mod system;

use crate::commands::Output;
use crate::commands::use_version::run;
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

pub(super) const BASE_PATH: &str = "/usr/bin:/bin";
pub(super) const N18: &str = "/n/versions/node/v18.20.4";
pub(super) const HASH: &str = "hash -r 2>/dev/null || true\n";

/// Each version directory with the `npm --version` it answers, if any.
const VERSIONS: [(&str, Option<&str>); 6] = [
    (N18, Some("10.7.0")),
    ("/n/versions/node/v18.19.0", Some("10.2.3")),
    ("/n/versions/node/v20.11.1", Some("10.2.4")),
    ("/n/versions/node/v22.3.0", None),
    ("/n/versions/io.js/v3.3.1", Some("2.14.3")),
    ("/n/v0.10.48", Some("2.15.1")),
];

pub(super) fn lab() -> FakeFileSystem {
    let aliases = [
        ("default", "18"),
        ("lts/*", "lts/iron"),
        ("lts/iron", "v20.11.1"),
        ("lts/hydrogen", "v18.20.4"),
    ];
    let fs = VERSIONS
        .iter()
        .fold(FakeFileSystem::default(), |fs, (dir, npm)| {
            let fs = fs.with_executable(&format!("{dir}/bin/node"), "#!/bin/sh");
            match npm {
                Some(_) => fs.with_executable(&format!("{dir}/bin/npm"), "#!/bin/sh"),
                None => fs,
            }
        });
    aliases
        .iter()
        .fold(fs, |fs, (name, target)| {
            fs.with_file(&format!("/n/alias/{name}"), &format!("{target}\n"))
        })
        .with_dir("/proj")
}

pub(super) fn npm_versions() -> FakeProcess {
    VERSIONS
        .iter()
        .filter_map(|(dir, npm)| npm.map(|version| (dir, version)))
        .fold(FakeProcess::default(), |process, (dir, version)| {
            process.with_success(
                &format!("{dir}/bin/npm"),
                "--version",
                &format!("{version}\n"),
            )
        })
}

pub(super) fn env_on(path: &str) -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path)
        .with_var("PWD", "/proj")
}

pub(super) fn use_in(
    fs: &FakeFileSystem,
    env: &FakeEnv,
    process: &FakeProcess,
    args: &[&str],
) -> Output {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let context = Context::new(fs, env).with_process(process);
    run(&context, &args).expect("nvm use runs")
}

fn use_version(args: &[&str]) -> Output {
    use_in(&lab(), &env_on(BASE_PATH), &npm_versions(), args)
}

/// The code of a switch to `directory` from `BASE_PATH`, with no `manpath`.
pub(super) fn switched_to(directory: &str) -> String {
    format!(
        "export PATH='{directory}/bin:{BASE_PATH}'\n{HASH}\
export NVM_BIN='{directory}/bin'\nexport NVM_INC='{directory}/include/node'\n"
    )
}

fn assert_switched(output: &Output, directory: &str, stdout: &str) {
    assert_eq!(output.stdout, stdout);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.script.render(), switched_to(directory));
}

#[test]
fn use_18_switches_to_the_highest_18_and_names_its_npm() {
    let output = use_version(&["18"]);
    assert_switched(&output, N18, "Now using node v18.20.4 (npm v10.7.0)");
}

#[test]
fn use_a_partial_version_with_a_v_switches_to_its_match() {
    let output = use_version(&["v18.19"]);
    let directory = "/n/versions/node/v18.19.0";
    assert_switched(&output, directory, "Now using node v18.19.0 (npm v10.2.3)");
}

#[test]
fn use_node_without_an_npm_has_no_npm_suffix() {
    let output = use_version(&["node"]);
    let directory = "/n/versions/node/v22.3.0";
    assert_switched(&output, directory, "Now using node v22.3.0");
}

#[test]
fn use_iojs_names_io_js_without_its_prefix() {
    let output = use_version(&["iojs"]);
    let directory = "/n/versions/io.js/v3.3.1";
    assert_switched(&output, directory, "Now using io.js v3.3.1 (npm v2.14.3)");
}

#[test]
fn use_silent_switches_and_prints_nothing() {
    let output = use_version(&["18", "--silent"]);
    assert_switched(&output, N18, "");
}

#[test]
fn use_lts_switches_to_the_lts_alias() {
    let output = use_version(&["--lts"]);
    let directory = "/n/versions/node/v20.11.1";
    assert_switched(&output, directory, "Now using node v20.11.1 (npm v10.2.4)");
    let hydrogen = use_version(&["--lts=hydrogen"]);
    assert_switched(&hydrogen, N18, "Now using node v18.20.4 (npm v10.7.0)");
}

#[test]
fn use_default_follows_the_alias() {
    let output = use_version(&["default"]);
    assert_switched(&output, N18, "Now using node v18.20.4 (npm v10.7.0)");
}

#[test]
fn use_a_version_below_0_12_uses_the_old_layout() {
    let output = use_version(&["0.10"]);
    assert_switched(
        &output,
        "/n/v0.10.48",
        "Now using node v0.10.48 (npm v2.15.1)",
    );
}

#[test]
fn use_18_while_io_js_is_active_replaces_its_entry() {
    let path = format!("/n/versions/io.js/v3.3.1/bin:{BASE_PATH}");
    let output = use_in(&lab(), &env_on(&path), &npm_versions(), &["18"]);
    assert_switched(&output, N18, "Now using node v18.20.4 (npm v10.7.0)");
}

#[test]
fn use_18_twice_says_it_again_and_keeps_the_path() {
    let path = format!("{N18}/bin:{BASE_PATH}");
    let output = use_in(&lab(), &env_on(&path), &npm_versions(), &["18"]);
    assert_switched(&output, N18, "Now using node v18.20.4 (npm v10.7.0)");
}

#[test]
fn an_npm_that_prints_nothing_gives_no_suffix() {
    let process = FakeProcess::default().with_success(&format!("{N18}/bin/npm"), "--version", "\n");
    let output = use_in(&lab(), &env_on(BASE_PATH), &process, &["18"]);
    assert_switched(&output, N18, "Now using node v18.20.4");
}
