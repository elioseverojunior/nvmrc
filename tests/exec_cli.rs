//! End-to-end: `nvm exec` and `nvm run` run fake `node` scripts from a real
//! `$NVM_DIR`, after their own messages, and exit with the child's status.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BINARY: &str = env!("CARGO_BIN_EXE_nvm");
const RUNNING_18: &str = "Running node v18.20.4 (npm v10.7.0)\n";

struct Fixture {
    root: tempfile::TempDir,
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A `node` that prints its version for `--version`, exits with N for
/// `exit N`, and otherwise prints its arguments.
fn fake_node(version: &str) -> String {
    format!(
        "case \"$1\" in\n--version) echo {version} ;;\n\
exit) echo \"node {version} exiting $2\"; exit \"$2\" ;;\n\
*) echo \"node {version} argv: $*\" ;;\nesac"
    )
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let fixture = Self { root };
        fixture.version("v18.20.4", "10.7.0");
        fixture.version("v20.11.1", "10.2.4");
        let alias = fixture.nvm_dir().join("alias/lts");
        fs::create_dir_all(&alias).unwrap();
        fs::write(alias.join("*"), "lts/iron\n").unwrap();
        fs::write(alias.join("iron"), "v20.11.1\n").unwrap();
        fs::create_dir_all(fixture.project()).unwrap();
        fixture
    }

    fn version(&self, version: &str, npm: &str) {
        let bin = self.nvm_dir().join(format!("versions/node/{version}/bin"));
        fs::create_dir_all(&bin).unwrap();
        script(&bin.join("node"), &fake_node(version));
        script(&bin.join("npm"), &format!("echo {npm}"));
    }

    fn nvm_dir(&self) -> PathBuf {
        self.root.path().join("nvm")
    }

    fn project(&self) -> PathBuf {
        self.root.path().join("proj")
    }

    fn path(&self) -> String {
        format!("{}:/usr/bin:/bin", self.root.path().join("empty").display())
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("NVM_DIR", self.nvm_dir())
            .env("PATH", self.path())
            .env("HOME", self.root.path())
            .env("PWD", self.project())
            .current_dir(self.project());
        command
    }

    fn nvm(&self, args: &[&str]) -> Output {
        self.command(BINARY).args(args).output().unwrap()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn exec_prints_running_before_the_output_of_the_command() {
    let output = Fixture::new().nvm(&["exec", "18", "node", "a", "b"]);
    assert_eq!(
        stdout(&output),
        format!("{RUNNING_18}node v18.20.4 argv: a b\n")
    );
    assert_eq!(stderr(&output), "");
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exec_exits_with_the_status_of_the_command() {
    let output = Fixture::new().nvm(&["exec", "18", "node", "exit", "7"]);
    let expected = format!("{RUNNING_18}node v18.20.4 exiting 7\n");
    assert_eq!((stdout(&output), output.status.code()), (expected, Some(7)));
}

#[test]
fn run_runs_node_with_the_arguments() {
    let output = Fixture::new().nvm(&["run", "18", "x"]);
    let expected = format!("{RUNNING_18}node v18.20.4 argv: x\n");
    assert_eq!((stdout(&output), output.status.code()), (expected, Some(0)));
}

#[test]
fn run_exits_with_the_status_of_node() {
    let output = Fixture::new().nvm(&["run", "--silent", "18", "exit", "3"]);
    let expected = "node v18.20.4 exiting 3\n".to_owned();
    assert_eq!((stdout(&output), output.status.code()), (expected, Some(3)));
}

#[test]
fn a_command_that_is_not_found_is_127() {
    let output = Fixture::new().nvm(&["exec", "18", "no-such-command-xyz"]);
    assert_eq!(stdout(&output), RUNNING_18);
    assert_eq!(stderr(&output), "nvm: no-such-command-xyz: not found\n");
    assert_eq!(output.status.code(), Some(127));
}

#[test]
fn silent_prints_only_the_output_of_the_command() {
    let output = Fixture::new().nvm(&["exec", "--silent", "18", "node", "a"]);
    assert_eq!(stdout(&output), "node v18.20.4 argv: a\n");
}

#[test]
fn lts_runs_the_latest_lts() {
    let output = Fixture::new().nvm(&["exec", "--lts", "node", "a"]);
    let expected = "Running node latest LTS -> v20.11.1 (npm v10.2.4)\nnode v20.11.1 argv: a\n";
    assert_eq!(stdout(&output), expected);
}

#[test]
fn a_command_without_a_version_runs_on_the_nvmrc_version_found_from_pwd() {
    let fixture = Fixture::new();
    fs::write(fixture.project().join(".nvmrc"), "20\n").unwrap();
    let output = fixture.nvm(&["exec", "npm", "-v"]);
    let found = format!(
        "Found '{}/.nvmrc' with version <20>",
        fixture.project().display()
    );
    let expected = format!("{found}\nRunning node v20.11.1 (npm v10.2.4)\n10.2.4\n");
    assert_eq!(stdout(&output), expected);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn the_child_sees_the_environment_of_the_version() {
    let fixture = Fixture::new();
    let show = "echo \"$NODE_VERSION|$NVM_BIN|${PATH%%:*}|[$NVM_CD_FLAGS]|$NVM_DIR\"";
    let output = fixture.nvm(&["exec", "--silent", "18", "sh", "-c", show]);
    let nvm_dir = fixture.nvm_dir();
    let bin = nvm_dir.join("versions/node/v18.20.4/bin");
    let (bin, nvm_dir) = (bin.display(), nvm_dir.display());
    assert_eq!(
        stdout(&output),
        format!("v18.20.4|{bin}|{bin}|[]|{nvm_dir}\n")
    );
}

#[test]
fn the_environment_of_the_caller_is_left_alone() {
    let fixture = Fixture::new();
    let program = "\"$BIN\" exec --silent 18 node a >/dev/null; echo \"$PATH|${NVM_BIN-unset}\"";
    let output = fixture
        .command("/bin/sh")
        .env("BIN", BINARY)
        .args(["-c", program])
        .output()
        .unwrap();
    assert_eq!(stdout(&output), format!("{}|unset\n", fixture.path()));
}
