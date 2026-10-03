//! An install activates its version (digest 9, steps 5 and 6) only in the
//! `nvm` function: `nvm use` for a version already there,
//! `nvm_use_if_needed` after a fresh install; a failing `use` sets the
//! status. Standalone, nothing is activated.

use super::nvmrc::{SHELL, V18_NODE};
use super::*;

const PREFIX_REFUSED: &str = "nvm is not compatible with the \"PREFIX\" environment variable: \
currently set to \"/zz\"\nRun `unset PREFIX` to unset it.";

fn world_in(fs: FakeFileSystem, vars: &[(&str, &str)]) -> World {
    let all: Vec<(&str, &str)> = SHELL.iter().chain(vars).copied().collect();
    World { fs, ..World::new() }.with_env(&all).in_function()
}

fn v18_installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable(V18_NODE, "x")
        .with_file("/n/alias/default", "18\n")
        .with_file("/p/.nvmrc", "18\n")
}

#[test]
fn standalone_an_installed_version_is_not_activated() {
    let world = World {
        fs: v18_installed(),
        ..World::new()
    }
    .with_env(&[("PWD", "/p/sub"), ("PATH", "/usr/bin")]);
    let output = world.run("").unwrap();
    assert_eq!(output.stdout, "Found '/p/.nvmrc' with version <18>");
    assert_eq!(output.stderr, "v18.19.0 is already installed.");
    assert!(output.script.is_empty());
}

#[test]
fn a_fresh_install_is_used_before_the_default_alias_is_made() {
    let world = world_in(FakeFileSystem::default(), &[]);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Now using node v20.10.0",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    let script = output.script.render();
    assert!(script.contains("export PATH='/n/versions/node/v20.10.0/bin:/usr/bin'"));
}

#[test]
fn a_fresh_install_of_the_version_in_use_is_not_used_again() {
    let fs = FakeFileSystem::default().with_file(NODE, "");
    let world = World { fs, ..World::new() }
        .with_env(&[("PWD", "/p"), ("PATH", "/n/versions/node/v20.10.0/bin")])
        .in_function();
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.stdout.contains("Now using"), "{}", output.stdout);
    assert!(output.script.is_empty());
    assert!(world.installed());
}

#[test]
fn a_failed_use_of_a_fresh_install_is_its_status_and_ends_it() {
    let world = world_in(FakeFileSystem::default(), &[("PREFIX", "/zz")]);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert!(output.stderr.ends_with(PREFIX_REFUSED), "{}", output.stderr);
    assert_eq!(output.stdout, "Downloading and installing node v20.10.0...");
    assert!(world.text("/n/alias/default").is_none());
    assert_eq!(output.script.render(), "unset NVM_BIN\nunset NVM_INC\n");
}

#[test]
fn a_failed_use_of_an_installed_version_skips_npm_and_the_alias_but_not_the_default() {
    let fs = FakeFileSystem::default()
        .with_executable(V18_NODE, "x")
        .with_file("/n/default-packages", "left-pad\n");
    let world = world_in(fs, &[("PREFIX", "/zz")]);
    let output = world.run("--alias=work 18").unwrap();
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(
        output.stderr,
        format!("v18.19.0 is already installed.\n{PREFIX_REFUSED}")
    );
    assert_eq!(
        output.stdout,
        "Creating default alias: default -> 18 (-> v18.19.0 *)"
    );
    assert!(world.text("/n/alias/work").is_none());
}

#[test]
fn save_after_a_failed_use_is_the_status_and_lets_the_alias_through() {
    let world = world_in(v18_installed(), &[("PREFIX", "/zz")]);
    let output = world.run("--save --alias=work 18").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Wrote version number (v18.19.0) to .nvmrc",
            "work -> 18 (-> v18.19.0 *)",
        ]
    );
}

#[test]
fn an_alias_follows_the_use_of_an_installed_version() {
    let world = world_in(v18_installed(), &[]);
    let output = world.run("--alias=work 18").unwrap();
    assert_eq!(
        lines(&output.stdout),
        ["Now using node v18.19.0", "work -> 18 (-> v18.19.0 *)"]
    );
    assert!(
        output
            .script
            .render()
            .contains("/n/versions/node/v18.19.0/bin")
    );
}
