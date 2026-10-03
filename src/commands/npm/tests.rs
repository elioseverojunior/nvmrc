use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn context<'a>(fs: &'a FakeFileSystem, env: &'a FakeEnv, process: &'a FakeProcess) -> Context<'a> {
    Context::new(fs, env).with_process(process)
}

#[test]
fn a_version_has_an_npm_when_bin_npm_is_a_file() {
    let fs = FakeFileSystem::default().with_executable(NPM, "#!/bin/sh");
    let (env, process) = (FakeEnv::default(), FakeProcess::default());
    let context = context(&fs, &env, &process);
    assert!(Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).is_some());
    assert!(Npm::in_version(&context, Path::new("/n/versions/node/v18.19.0")).is_none());
}

#[test]
fn the_npm_on_path_is_the_first_one() {
    let fs = FakeFileSystem::default()
        .with_executable("/a/bin/npm", "")
        .with_executable("/b/bin/npm", "");
    let env = FakeEnv::default().with_var("PATH", "/x:/a/bin:/b/bin");
    let process = FakeProcess::default();
    let npm = Npm::on_path(&context(&fs, &env, &process)).unwrap();
    assert_eq!(npm.program, Path::new("/a/bin/npm"));
    assert_eq!(npm.bin_dir, Path::new("/a/bin"));
}

#[test]
fn it_runs_with_its_own_bin_directory_first_on_the_path() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let process = FakeProcess::default().with_success(NPM, "--version", "10.2.3\n");
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    assert_eq!(npm.version(&context).as_deref(), Some("10.2.3"));
    let ran = process.executed();
    assert_eq!(
        ran[0].path_prefix.as_deref(),
        Some(Path::new("/n/versions/node/v20.10.0/bin"))
    );
}

#[test]
fn a_version_that_is_empty_or_fails_is_none() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let silent = FakeProcess::default().with_success(NPM, "--version", "\n");
    let failing = FakeProcess::default().with_execution(NPM, "--version", Completed::default());
    for process in [&silent, &failing, &FakeProcess::default()] {
        let context = context(&fs, &env, process);
        let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
        assert_eq!(npm.version(&context), None);
    }
}

#[test]
fn run_passes_the_output_on_and_says_whether_it_worked() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let done = Completed {
        success: false,
        stdout: "out1\nout2\n".to_owned(),
        stderr: "oops\n".to_owned(),
    };
    let process = FakeProcess::default().with_execution(NPM, "install -g left-pad", done);
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    assert!(!npm.run(&context, &["install", "-g", "left-pad"], &mut transcript));
    let output = transcript.finish(crate::error::NvmExitCode::Success);
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        ("out1\nout2", "oops")
    );
}

#[test]
fn a_program_that_cannot_start_is_reported_and_is_a_failure() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let (env, process) = (FakeEnv::default(), FakeProcess::default());
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    assert!(!npm.run(&context, &["list"], &mut transcript));
    let stderr = transcript.finish(crate::error::NvmExitCode::Success).stderr;
    assert!(stderr.starts_with("/n/versions/node/v20.10.0/bin/npm: "));
    assert_eq!(npm.output(&context, &["list"]), None);
}

#[test]
fn output_returns_what_a_successful_run_printed() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let process = FakeProcess::default().with_success(NPM, "list -g --depth=0", "tree\n");
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    assert_eq!(
        npm.output(&context, &["list", "-g", "--depth=0"])
            .as_deref(),
        Some("tree\n")
    );
}
