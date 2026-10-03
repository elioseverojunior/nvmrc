//! `nvm use` resolution, one test per observed row of the oracle (digest
//! 4.3 and 3.5), against the lab of the digest: node 18.20.4, 18.19.0,
//! 20.11.1 and 22.3.0, io.js 3.3.1, the old layout `$NVM_DIR/v0.10.48`, and
//! its aliases.

mod failures;
mod nvmrc;
mod save;

use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::commands::use_version::{Halt, SystemFlavor, SystemNode, Target, plan};
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};

pub(super) const BASE_PATH: &str = "/usr/bin:/bin";

pub(super) fn lab() -> FakeFileSystem {
    let versions = [
        "/n/versions/node/v18.20.4",
        "/n/versions/node/v18.19.0",
        "/n/versions/node/v20.11.1",
        "/n/versions/node/v22.3.0",
        "/n/versions/io.js/v3.3.1",
        "/n/v0.10.48",
    ];
    let aliases = [
        ("default", "18"),
        ("lts/*", "lts/iron"),
        ("lts/iron", "v20.11.1"),
        ("lts/hydrogen", "v18.20.4"),
        ("myalias", "20"),
        ("loopa", "loopb"),
        ("loopb", "loopa"),
        ("missing", "99"),
    ];
    let fs = versions.iter().fold(FakeFileSystem::default(), |fs, dir| {
        fs.with_executable(&format!("{dir}/bin/node"), "#!/bin/sh")
    });
    aliases
        .iter()
        .fold(fs, |fs, (name, target)| {
            fs.with_file(&format!("/n/alias/{name}"), &format!("{target}\n"))
        })
        .with_executable("/n/versions/io.js/v3.3.1/bin/iojs", "#!/bin/sh")
        .with_dir("/proj/sub/deep")
}

pub(super) fn env_on(path: &str) -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path)
        .with_var("PWD", "/proj")
}

pub(super) fn run_in(
    fs: &FakeFileSystem,
    env: &FakeEnv,
    args: &[&str],
) -> (Result<Target, Halt>, Output) {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let mut transcript = Transcript::default();
    let planned = plan(&Context::new(fs, env), &args, &mut transcript);
    let status = planned
        .as_ref()
        .map_or_else(|halt| halt.status, |_| NvmExitCode::Success);
    (planned.map(|(_, target)| target), transcript.finish(status))
}

pub(super) fn run(args: &[&str]) -> (Result<Target, Halt>, Output) {
    run_in(&lab(), &env_on(BASE_PATH), args)
}

pub(super) fn version(text: &str) -> Version {
    text.parse().expect("valid version")
}

pub(super) fn installed(text: &str, provided: Option<&str>, directory: &str) -> Target {
    Target::Installed {
        version: version(text),
        from_nvmrc: false,
        provided: provided.map(str::to_owned),
        directory: PathBuf::from(directory),
    }
}

/// `nvm use <args>` picks `expected` and prints nothing.
fn assert_uses(args: &[&str], expected: &str) {
    let (target, output) = run(args);
    let Ok(Target::Installed {
        version: chosen, ..
    }) = target
    else {
        panic!("{args:?} gave {target:?} and {output:?}");
    };
    assert_eq!(chosen.to_string(), expected, "{args:?}");
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        ("", ""),
        "{args:?}"
    );
}

#[test]
fn a_major_picks_the_newest_installed_match_and_remembers_what_was_asked() {
    let (target, output) = run(&["18"]);
    let expected = installed("v18.20.4", Some("18"), "/n/versions/node/v18.20.4");
    assert_eq!(target, Ok(expected));
    assert_eq!(output, Output::default());
}

#[test]
fn partial_and_full_versions_pick_their_match() {
    assert_uses(&["v18.19"], "v18.19.0");
    assert_uses(&["18.19.0"], "v18.19.0");
    assert_uses(&["v18.20.4"], "v18.20.4");
}

#[test]
fn node_stable_22_and_v_all_pick_the_newest_node() {
    for name in ["node", "stable", "22", "v"] {
        assert_uses(&[name], "v22.3.0");
    }
}

#[test]
fn iojs_in_both_spellings_picks_the_newest_iojs_in_its_directory() {
    for name in ["iojs", "io.js"] {
        let (target, _) = run(&[name]);
        let expected = installed("iojs-v3.3.1", Some(name), "/n/versions/io.js/v3.3.1");
        assert_eq!(target, Ok(expected));
    }
}

#[test]
fn aliases_are_followed() {
    assert_uses(&["default"], "v18.20.4");
    assert_uses(&["myalias"], "v20.11.1");
    assert_uses(&["lts/*"], "v20.11.1");
    assert_uses(&["lts/hydrogen"], "v18.20.4");
}

#[test]
fn an_old_version_lives_directly_in_nvm_dir() {
    let (target, _) = run(&["0.10"]);
    assert_eq!(
        target,
        Ok(installed("v0.10.48", Some("0.10"), "/n/v0.10.48"))
    );
}

#[test]
fn lts_flags_resolve_their_alias_and_provide_no_version() {
    let (target, _) = run(&["--lts"]);
    let expected = installed("v20.11.1", None, "/n/versions/node/v20.11.1");
    assert_eq!(target, Ok(expected));
    assert_uses(&["--lts=hydrogen"], "v18.20.4");
}

#[test]
fn flags_anywhere_and_the_last_positional_decide() {
    assert_uses(&["18", "--silent"], "v18.20.4");
    assert_uses(&["--bogus", "18"], "v18.20.4");
    assert_uses(&["18", "20"], "v20.11.1");
    assert_uses(&["--", "18"], "v18.20.4");
}

#[test]
fn current_is_the_active_version() {
    let env = env_on("/n/versions/node/v18.20.4/bin:/usr/bin:/bin");
    let (target, output) = run_in(&lab(), &env, &["current"]);
    let expected = installed("v18.20.4", Some("current"), "/n/versions/node/v18.20.4");
    assert_eq!(target, Ok(expected));
    assert_eq!(output, Output::default());
}

fn system_target(flavor: SystemFlavor, binary: &str) -> Target {
    Target::System(SystemNode {
        flavor,
        binary: PathBuf::from(binary),
    })
}

#[test]
fn system_is_the_node_left_once_the_nvm_entries_are_stripped() {
    let fs = lab().with_executable("/sys/bin/node", "");
    let env = env_on("/n/versions/node/v18.20.4/bin:/sys/bin:/usr/bin");
    let (target, output) = run_in(&fs, &env, &["system"]);
    assert_eq!(
        target,
        Ok(system_target(SystemFlavor::Node, "/sys/bin/node"))
    );
    assert_eq!(output, Output::default());
    let (silent, _) = run_in(&fs, &env, &["system", "--silent"]);
    assert_eq!(
        silent,
        Ok(system_target(SystemFlavor::Node, "/sys/bin/node"))
    );
}

#[test]
fn a_system_iojs_counts_when_there_is_no_system_node() {
    let fs = lab().with_executable("/sys/bin/iojs", "");
    let (target, _) = run_in(&fs, &env_on("/sys/bin:/usr/bin"), &["system"]);
    assert_eq!(
        target,
        Ok(system_target(SystemFlavor::IoJs, "/sys/bin/iojs"))
    );
}

#[test]
fn the_options_come_back_with_the_target() {
    let args = ["--delete-prefix".to_owned(), "18".to_owned()];
    let (fs, env) = (lab(), env_on(BASE_PATH));
    let mut transcript = Transcript::default();
    let (options, _) = plan(&Context::new(&fs, &env), &args, &mut transcript).expect("planned");
    assert!(options.delete_prefix);
    assert_eq!(options.version.as_deref(), Some("18"));
}
