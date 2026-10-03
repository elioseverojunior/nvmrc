use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

fn world() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable("/n/versions/node/v20.10.0/bin/node", "x")
        .with_executable("/n/versions/node/v18.19.0/bin/node", "x")
        .with_file("/n/.cache/bin/node-v20.10.0-linux-x64/files/top/f", "x")
        .with_file(
            "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "t",
        )
}

fn run_in(fs: &FakeFileSystem, path: &str, line: &str) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    run(&Context::new(fs, &env), &words)
}

fn exists(fs: &FakeFileSystem, path: &str) -> bool {
    fs.file_info(Path::new(path)).is_ok()
}

#[test]
fn it_removes_the_version_and_its_unpack_leftovers_but_keeps_the_archive() {
    let fs = world();
    let output = run_in(&fs, "", "20").unwrap();
    assert_eq!(output.stdout, "Uninstalled node v20.10.0");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!exists(&fs, "/n/versions/node/v20.10.0"));
    assert!(!exists(&fs, "/n/.cache/bin/node-v20.10.0-linux-x64/files"));
    assert!(exists(
        &fs,
        "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz"
    ));
    assert!(exists(&fs, "/n/versions/node/v18.19.0/bin/node"));
}

#[test]
fn aliases_that_name_the_version_are_deleted_and_others_stay() {
    let fs = world()
        .with_file("/n/alias/bar", "v20.10.0\n")
        .with_file("/n/alias/foo", "20\n")
        .with_file("/n/alias/also", "# note\nv20.10.0\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n");
    let output = run_in(&fs, "", "v20.10.0").unwrap();
    assert_eq!(
        output.stdout,
        "Uninstalled node v20.10.0\n\
         Deleted alias also - restore it with `nvm alias \"also\" \"v20.10.0\"`\n\
         Deleted alias bar - restore it with `nvm alias \"bar\" \"v20.10.0\"`"
    );
    assert!(exists(&fs, "/n/alias/foo"));
    assert!(exists(&fs, "/n/alias/lts/iron"));
}

#[test]
fn lts_options_resolve_through_the_lts_aliases() {
    for line in ["--lts", "lts/*", "--lts=iron", "lts/iron"] {
        let fs = world()
            .with_file("/n/alias/lts/*", "lts/iron\n")
            .with_file("/n/alias/lts/iron", "v20.10.0\n");
        let output = run_in(&fs, "", line).unwrap();
        assert_eq!(
            output.stdout.lines().next(),
            Some("Uninstalled node v20.10.0"),
            "{line}"
        );
    }
}

#[test]
fn a_version_that_is_not_installed_is_a_message_with_status_0() {
    let fs = world();
    let output = run_in(&fs, "", "99").unwrap();
    assert_eq!(output.stderr, "Version '99' is not installed.");
    assert_eq!(
        (output.stdout.as_str(), output.status),
        ("", NvmExitCode::Success)
    );
    let lts = run_in(&FakeFileSystem::default(), "", "--lts").unwrap();
    assert_eq!(lts.stderr, "Version '--lts' is not installed.");
}

#[test]
fn a_directory_without_a_runnable_node_is_not_installed_and_is_left_alone() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/README", "x");
    let output = run_in(&fs, "", "20").unwrap();
    assert_eq!(
        output.stderr,
        "Version 'v20.10.0' (inferred from 20) is not installed."
    );
    assert!(exists(&fs, "/n/versions/node/v20.10.0/README"));
}

#[test]
fn the_version_in_use_cannot_be_uninstalled() {
    let fs = world();
    let path = "/n/versions/node/v20.10.0/bin";
    for line in ["20", "v20.10.0"] {
        let error = run_in(&fs, path, line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
        assert_eq!(
            error.to_string(),
            format!(
                "nvm: Cannot uninstall currently-active node version, v20.10.0 (inferred from {line})."
            )
        );
    }
    assert!(exists(&fs, "/n/versions/node/v20.10.0/bin/node"));
}

#[test]
fn an_iojs_version_is_named_as_such() {
    let fs = FakeFileSystem::default().with_executable("/n/versions/io.js/v3.3.1/bin/node", "x");
    let active = run_in(&fs, "/n/versions/io.js/v3.3.1/bin", "iojs-v3.3.1").unwrap_err();
    assert!(
        active
            .to_string()
            .contains("currently-active io.js version, iojs-v3.3.1")
    );
    let output = run_in(&fs, "", "iojs-v3.3.1").unwrap();
    assert_eq!(output.stdout, "Uninstalled io.js v3.3.1");
}

#[test]
fn anything_but_one_word_is_the_usage_with_status_127() {
    let fs = world();
    for line in ["", "18 20"] {
        let error = run_in(&fs, "", line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
        assert!(
            error
                .to_string()
                .starts_with("Usage: nvm uninstall <version>\n       nvm uninstall --lts\n")
        );
    }
}
