//! End-to-end: `nvm use` and `nvm deactivate` against a real `$NVM_DIR`, both
//! standalone and through the shell-code channel of a real `sh`.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BINARY: &str = env!("CARGO_BIN_EXE_nvm");
const USING: &str = "Now using node v18.20.4 (npm v10.7.0)\n";
/// What the generated `nvm` function does around the binary.
const SHELL_FUNCTION: &str = r#"
nvm() {
  { code=$(NVMRC_SCRIPT_FD=3 "$BIN" "$@" 3>&1 1>&4 4>&-); } 4>&1
  status=$?
  eval "$code"
  return "$status"
}
"#;

struct Fixture {
    root: tempfile::TempDir,
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fixture {
    fn new(with_manpath: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("nvm/versions/node/v18.20.4/bin");
        fs::create_dir_all(&bin).unwrap();
        script(&bin.join("node"), "echo v18.20.4");
        script(&bin.join("npm"), "echo 10.7.0");
        fs::create_dir_all(root.path().join("path")).unwrap();
        if with_manpath {
            script(&root.path().join("path/manpath"), "echo /usr/share/man");
        }
        Self { root }
    }

    fn nvm_dir(&self) -> PathBuf {
        self.root.path().join("nvm")
    }

    fn path_dir(&self) -> PathBuf {
        self.root.path().join("path")
    }

    fn version_bin(&self) -> String {
        format!("{}/versions/node/v18.20.4/bin", self.nvm_dir().display())
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("NVM_DIR", self.nvm_dir())
            .env("PATH", self.path_dir())
            .env("HOME", self.root.path());
        command
    }

    fn nvm(&self, args: &[&str]) -> Output {
        self.command(BINARY).args(args).output().unwrap()
    }

    fn shell(&self, commands: &str) -> Output {
        let program = format!("{SHELL_FUNCTION}\n{commands}");
        self.command("/bin/sh")
            .env("BIN", BINARY)
            .args(["-c", &program])
            .output()
            .unwrap()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn standalone_use_prints_the_code_on_stdout_and_the_message_on_stderr() {
    let fixture = Fixture::new(false);
    let output = fixture.nvm(&["use", "18"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(text(&output.stderr), USING);
    let bin = fixture.version_bin();
    let expected = format!(
        "export PATH='{bin}:{path}'\nhash -r 2>/dev/null || true\n\
         export NVM_BIN='{bin}'\nexport NVM_INC='{inc}'\n",
        path = fixture.path_dir().display(),
        inc = bin.replace("/bin", "/include/node"),
    );
    assert_eq!(text(&output.stdout), expected);
}

#[test]
fn standalone_use_also_exports_manpath_when_a_manpath_program_exists() {
    let fixture = Fixture::new(true);
    let output = fixture.nvm(&["use", "18"]);
    assert_eq!(text(&output.stderr), USING);
    let stdout = text(&output.stdout);
    assert!(stdout.starts_with("export MANPATH='"), "{stdout}");
    assert!(
        stdout.contains("/versions/node/v18.20.4/share/man:"),
        "{stdout}"
    );
    assert!(stdout.contains("export PATH='"), "{stdout}");
}

#[test]
fn use_of_a_missing_version_exits_3_with_nothing_on_stdout() {
    let fixture = Fixture::new(false);
    let output = fixture.nvm(&["use", "99"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(text(&output.stderr).contains("99"));
}

#[test]
fn deactivate_prints_the_code_that_undoes_the_use() {
    let fixture = Fixture::new(false);
    let bin = fixture.version_bin();
    let output = fixture
        .command(BINARY)
        .arg("deactivate")
        .env("PATH", format!("{bin}:/usr/bin"))
        .env("NVM_BIN", &bin)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("unset NVM_BIN\n"), "{stdout}");
    assert!(stdout.contains("export PATH='/usr/bin'"), "{stdout}");
}

#[test]
fn through_the_shell_function_the_message_stays_on_stdout_and_the_path_changes() {
    let fixture = Fixture::new(false);
    let output = fixture.shell("nvm use 18; echo \"status=$?\"; echo \"$PATH\"");
    let expected = format!(
        "{USING}status=0\n{}:{}\n",
        fixture.version_bin(),
        fixture.path_dir().display()
    );
    assert_eq!(text(&output.stdout), expected);
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn silencing_stdout_keeps_the_path_change() {
    let fixture = Fixture::new(false);
    let output = fixture.shell("nvm use 18 >/dev/null; echo \"$PATH\"");
    let expected = format!(
        "{}:{}\n",
        fixture.version_bin(),
        fixture.path_dir().display()
    );
    assert_eq!(text(&output.stdout), expected);
}

#[test]
fn a_failing_use_leaves_the_path_alone_and_returns_its_status() {
    let fixture = Fixture::new(false);
    let output = fixture.shell("nvm use 99; echo \"status=$?\"; echo \"$PATH\"");
    let stdout = text(&output.stdout);
    assert_eq!(
        stdout,
        format!("status=3\n{}\n", fixture.path_dir().display())
    );
    assert!(text(&output.stderr).contains("99"));
}

#[test]
fn deactivate_through_the_shell_function_removes_the_version_from_the_path() {
    let fixture = Fixture::new(false);
    let output = fixture
        .shell("nvm use 18 >/dev/null; nvm deactivate >/dev/null; echo \"$PATH|${NVM_BIN-unset}\"");
    assert_eq!(
        text(&output.stdout),
        format!("{}|unset\n", fixture.path_dir().display())
    );
}
