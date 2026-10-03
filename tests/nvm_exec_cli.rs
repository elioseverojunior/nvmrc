//! End-to-end: `nvm-exec <command>` selects a node from `NODE_VERSION` or the
//! `.nvmrc` in a real `$NVM_DIR` with fake `node` scripts, then runs the
//! command with its exit status.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BINARY: &str = env!("CARGO_BIN_EXE_nvm-exec");
const UNABLE: &str = "nvm-exec: unable to select a node version\n  Set `NODE_VERSION` \
(e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.\n";

struct Fixture {
    root: tempfile::TempDir,
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A `node` that exits with N for `exit N` and otherwise prints its
/// arguments.
fn fake_node(version: &str) -> String {
    format!(
        "case \"$1\" in\nexit) echo \"node {version} exiting $2\"; exit \"$2\" ;;\n\
*) echo \"node {version} argv: $*\" ;;\nesac"
    )
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            root: tempfile::tempdir().unwrap(),
        };
        for version in ["v18.20.4", "v20.11.1"] {
            let bin = fixture
                .nvm_dir()
                .join(format!("versions/node/{version}/bin"));
            fs::create_dir_all(&bin).unwrap();
            script(&bin.join("node"), &fake_node(version));
        }
        let alias = fixture.nvm_dir().join("alias/lts");
        fs::create_dir_all(&alias).unwrap();
        fs::write(alias.join("hydrogen"), "v18.20.4\n").unwrap();
        fs::write(fixture.nvm_dir().join("alias/default"), "18\n").unwrap();
        fs::create_dir_all(fixture.project()).unwrap();
        fixture
    }

    fn nvm_dir(&self) -> PathBuf {
        self.root.path().join("nvm")
    }

    fn project(&self) -> PathBuf {
        self.root.path().join("proj")
    }

    fn base_path(&self) -> String {
        format!("{}:/usr/bin:/bin", self.root.path().join("empty").display())
    }

    fn command(&self) -> Command {
        let mut command = Command::new(BINARY);
        command
            .env_clear()
            .env("NVM_DIR", self.nvm_dir())
            .env("PATH", self.base_path())
            .env("HOME", self.root.path())
            .env("PWD", self.project())
            .current_dir(self.project());
        command
    }

    fn nvm_exec(&self, node_version: Option<&str>, args: &[&str]) -> Output {
        let mut command = self.command();
        if let Some(version) = node_version {
            command.env("NODE_VERSION", version);
        }
        command.args(args).output().unwrap()
    }

    fn write_nvmrc(&self, text: &str) {
        fs::write(self.project().join(".nvmrc"), text).unwrap();
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_result(output: &Output, expected: (&str, &str, i32)) {
    let actual = (stdout(output), stderr(output), output.status.code());
    assert_eq!(
        actual,
        (expected.0.into(), expected.1.into(), Some(expected.2))
    );
}

#[test]
fn node_version_18_runs_the_command_silently() {
    let output = Fixture::new().nvm_exec(Some("18"), &["node", "a"]);
    assert_result(&output, ("node v18.20.4 argv: a\n", "", 0));
}

#[test]
fn node_version_lts_hydrogen_resolves_the_alias() {
    let output = Fixture::new().nvm_exec(Some("lts/hydrogen"), &["node", "a"]);
    assert_result(&output, ("node v18.20.4 argv: a\n", "", 0));
}

#[test]
fn node_version_default_puts_the_default_first_on_path() {
    let fixture = Fixture::new();
    let output = fixture.nvm_exec(Some("default"), &["sh", "-c", "echo $PATH"]);
    let expected = format!(
        "{}/versions/node/v18.20.4/bin:{}\n",
        fixture.nvm_dir().display(),
        fixture.base_path()
    );
    assert_result(&output, (&expected, "", 0));
}

#[test]
fn the_child_environment_keeps_node_version_as_given() {
    let show = "echo \"$NODE_VERSION|$NVM_BIN|[$NVM_CD_FLAGS]\"";
    let fixture = Fixture::new();
    let output = fixture.nvm_exec(Some("18"), &["sh", "-c", show]);
    let bin = fixture.nvm_dir().join("versions/node/v18.20.4/bin");
    assert_result(&output, (&format!("18|{}|[]\n", bin.display()), "", 0));
}

#[test]
fn node_version_99_is_127_with_the_not_installed_message() {
    let output = Fixture::new().nvm_exec(Some("99"), &["node", "a"]);
    let message = "N/A: version \"v99\" is not yet installed.\n\nYou need to run `nvm install \
99` to install and use it.\n";
    assert_result(&output, ("", message, 127));
}

#[test]
fn the_nvmrc_is_found_and_its_version_runs() {
    let fixture = Fixture::new();
    fixture.write_nvmrc("20\n");
    let output = fixture.nvm_exec(None, &["node", "a"]);
    let found = format!(
        "Found '{}/.nvmrc' with version <20>\n",
        fixture.project().display()
    );
    assert_result(&output, (&format!("{found}node v20.11.1 argv: a\n"), "", 0));
}

#[test]
fn an_nvmrc_that_is_not_installed_is_127_with_both_lines() {
    let fixture = Fixture::new();
    fixture.write_nvmrc("99\n");
    let output = fixture.nvm_exec(None, &["node", "a"]);
    let found = format!(
        "Found '{}/.nvmrc' with version <99>\n",
        fixture.project().display()
    );
    let not_installed = "N/A: version \"v99\" is not yet installed.\n\nYou need to run `nvm \
install 99` to install and use it.\n";
    assert_result(&output, (&found, &format!("{not_installed}{UNABLE}"), 127));
}

#[test]
fn nothing_available_is_127_even_with_a_version_active() {
    let fixture = Fixture::new();
    let active = fixture.nvm_dir().join("versions/node/v18.20.4/bin");
    let output = fixture
        .command()
        .env(
            "PATH",
            format!("{}:{}", active.display(), fixture.base_path()),
        )
        .args(["node", "a"])
        .output()
        .unwrap();
    let stderr_text = format!("No version provided and no .nvmrc file found\n{UNABLE}");
    assert_result(&output, ("", &stderr_text, 127));
}

#[test]
fn the_exit_status_of_the_command_is_the_exit_status() {
    let output = Fixture::new().nvm_exec(Some("18"), &["node", "exit", "7"]);
    assert_result(&output, ("node v18.20.4 exiting 7\n", "", 7));
}

#[test]
fn a_command_that_is_not_found_is_127() {
    let output = Fixture::new().nvm_exec(Some("18"), &["no-such-command-xyz"]);
    assert_result(
        &output,
        ("", "nvm-exec: no-such-command-xyz: not found\n", 127),
    );
}

#[test]
fn no_command_is_success() {
    let output = Fixture::new().nvm_exec(Some("18"), &[]);
    assert_result(&output, ("", "", 0));
}

#[test]
fn a_leading_double_dash_is_dropped() {
    let output = Fixture::new().nvm_exec(Some("18"), &["--", "node", "a"]);
    assert_result(&output, ("node v18.20.4 argv: a\n", "", 0));
}

#[test]
fn a_parent_that_sets_no_pwd_or_a_stale_one_still_finds_the_nvmrc_of_the_directory() {
    let fixture = Fixture::new();
    fixture.write_nvmrc("20\n");
    let real = fs::canonicalize(fixture.project()).unwrap();
    let expected = format!(
        "Found '{}/.nvmrc' with version <20>\nnode v20.11.1 argv: a\n",
        real.display()
    );
    let mut stale = fixture.command();
    stale.env("PWD", fixture.root.path()).args(["node", "a"]);
    assert_result(&stale.output().unwrap(), (&expected, "", 0));
    let mut missing = fixture.command();
    missing.env_remove("PWD").args(["node", "a"]);
    assert_result(&missing.output().unwrap(), (&expected, "", 0));
}
