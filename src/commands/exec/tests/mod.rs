//! `nvm exec` with fakes, against the lab of the digest: node 18.20.4,
//! 18.19.0, 20.11.1 and 22.3.0 (no npm), io.js 3.3.1, the old layout
//! `$NVM_DIR/v0.10.48`, a system node in `/sys/bin`, and the aliases.

mod environment;
mod parsing;
mod rows;

use crate::commands::Output;
use crate::commands::exec::run;
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Invocation;

pub(crate) const BASE_PATH: &str = "/usr/bin:/bin";
pub(crate) const N18: &str = "/n/versions/node/v18.20.4";
pub(crate) const N20: &str = "/n/versions/node/v20.11.1";
pub(crate) const ACTIVE_18: &str = "/n/versions/node/v18.20.4/bin:/usr/bin:/bin";
pub(crate) const WARNING: &str = "WARNING: `nvm exec` was invoked without a version argument \
and without an .nvmrc file.\n  Falling back to the active node version; this will become an \
error in a future release.\n  Pass `current` explicitly (e.g. `nvm exec current ...`) to \
silence this warning.";
pub(crate) const NO_NVMRC: &str = "No version provided and no .nvmrc file found";
pub(crate) const CURRENT_NOT_INSTALLED: &str = "N/A: version \"current\" is not yet installed.\n\n\
You need to run `nvm install current` to install and use it.";

/// Each version directory with the `npm --version` it answers, if any.
const VERSIONS: [(&str, Option<&str>); 7] = [
    (N18, Some("10.7.0")),
    ("/n/versions/node/v18.19.0", Some("10.2.3")),
    (N20, Some("10.2.4")),
    ("/n/versions/node/v22.3.0", None),
    ("/n/versions/io.js/v3.3.1", Some("2.14.3")),
    ("/n/v0.10.48", Some("2.15.1")),
    ("/sys", Some("8.0.0")),
];

pub(crate) fn lab() -> FakeFileSystem {
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
        .with_executable("/n/versions/io.js/v3.3.1/bin/iojs", "#!/bin/sh")
        .with_dir("/proj")
}

pub(crate) fn npm_versions() -> FakeProcess {
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

pub(crate) fn env_on(path: &str) -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path)
        .with_var("PWD", "/proj")
}

pub(crate) fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

pub(crate) fn exec_in(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Output {
    let process = npm_versions();
    let context = Context::new(fs, env).with_process(&process);
    run(&context, &strings(args)).expect("nvm exec runs")
}

/// `nvm exec` from `/proj` with nothing active and no `.nvmrc`.
pub(crate) fn exec(args: &[&str]) -> Output {
    exec_in(&lab(), &env_on(BASE_PATH), args)
}

/// What the child of a switch to `directory` from `path` runs with.
pub(crate) fn child_on(
    path: &str,
    directory: &str,
    node_version: &str,
    command: &[&str],
) -> Invocation {
    let new_path = if path == ACTIVE_18 {
        format!("{directory}/bin:/usr/bin:/bin")
    } else {
        format!("{directory}/bin:{path}")
    };
    Invocation::new(command[0])
        .args(&command[1..])
        .env("PATH", &new_path)
        .env("NVM_BIN", &format!("{directory}/bin"))
        .env("NVM_INC", &format!("{directory}/include/node"))
        .env("NODE_VERSION", node_version)
        .env("NVM_CD_FLAGS", "")
        .env("NVM_DIR", "/n")
}

/// The child of a switch from `BASE_PATH`.
pub(crate) fn child(directory: &str, node_version: &str, command: &[&str]) -> Invocation {
    child_on(BASE_PATH, directory, node_version, command)
}

pub(crate) fn assert_streams(output: &Output, stdout: &str, stderr: &str, status: NvmExitCode) {
    assert_eq!(output.stdout, stdout, "stdout");
    assert_eq!(output.stderr, stderr, "stderr");
    assert_eq!(output.status, status, "status");
}

/// A run that went ahead: its stdout, an empty stderr, and the child.
pub(crate) fn assert_runs(output: &Output, stdout: &str, child: Invocation) {
    assert_streams(output, stdout, "", NvmExitCode::Success);
    assert_eq!(output.spawn, Some(child));
}

/// A run that stopped before the child.
pub(crate) fn assert_stops(output: &Output, stdout: &str, stderr: &str, status: NvmExitCode) {
    assert_streams(output, stdout, stderr, status);
    assert_eq!(output.spawn, None);
}
