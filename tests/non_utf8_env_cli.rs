//! End-to-end: a `PATH`, `MANPATH` or `NODE_PATH` that is set but is not
//! UTF-8 is never rebuilt from nothing. The commands that would rewrite it
//! fail on stderr, with no shell code and no child started.
#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const NVM: &str = env!("CARGO_BIN_EXE_nvm");
const NVM_EXEC: &str = env!("CARGO_BIN_EXE_nvm-exec");

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `$NVM_DIR` with node v18.20.4 (whose `bin` also holds a `manpath`, so
/// `use` rewrites `MANPATH`), active on a `PATH` that ends in `/usr/bin:/bin`.
struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("nvm/versions/node/v18.20.4/bin");
        fs::create_dir_all(&bin).unwrap();
        script(&bin.join("node"), "echo v18.20.4");
        script(&bin.join("manpath"), "echo /usr/share/man");
        Self { root }
    }

    fn version_dir(&self) -> PathBuf {
        self.root.path().join("nvm/versions/node/v18.20.4")
    }

    /// The value of `name`: active entries of v18 around a byte that is not
    /// UTF-8.
    fn bad_value(&self, name: &str) -> OsString {
        let suffix = match name {
            "PATH" => "bin",
            "MANPATH" => "share/man",
            _ => "lib/node_modules",
        };
        let entry = self.version_dir().join(suffix);
        let mut bytes = entry.into_os_string().into_vec();
        bytes.extend_from_slice(b":/tmp/caf\xe9:/usr/bin:/bin");
        OsString::from_vec(bytes)
    }

    fn run(&self, program: &str, args: &[&str], name: &str) -> Output {
        let path = if name == "PATH" {
            self.bad_value("PATH")
        } else {
            format!("{}/bin:/usr/bin:/bin", self.version_dir().display()).into()
        };
        Command::new(program)
            .args(args)
            .env_clear()
            .env("NVM_DIR", self.root.path().join("nvm"))
            .env("HOME", self.root.path())
            .env("PWD", self.root.path())
            .env("NODE_VERSION", "18")
            .env("PATH", path)
            .env(name, self.bad_value(name))
            .current_dir(self.root.path())
            .output()
            .unwrap()
    }
}

fn assert_refused(output: &Output, name: &str, status: i32, case: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "", "{case}");
    assert!(
        stderr.contains(&format!("${name} is not valid UTF-8")),
        "{case}: {stderr}"
    );
    assert_eq!(output.status.code(), Some(status), "{case}: {stderr}");
}

const CHILD: [&str; 3] = ["/bin/sh", "-c", "echo ran"];

#[test]
fn use_and_deactivate_refuse_a_variable_they_would_rewrite() {
    let fixture = Fixture::new();
    let cases: [(&[&str], &str); 5] = [
        (&["use", "18"], "PATH"),
        (&["use", "18"], "MANPATH"),
        (&["deactivate"], "PATH"),
        (&["deactivate"], "MANPATH"),
        (&["deactivate"], "NODE_PATH"),
    ];
    for (args, name) in cases {
        let output = fixture.run(NVM, args, name);
        assert_refused(&output, name, 1, &format!("{args:?} {name}"));
    }
}

#[test]
fn exec_run_and_nvm_exec_start_no_child_with_a_rebuilt_variable() {
    let fixture = Fixture::new();
    let exec = [&["exec", "18"][..], &CHILD[..]].concat();
    for name in ["PATH", "MANPATH"] {
        let output = fixture.run(NVM, &exec, name);
        assert_refused(&output, name, 1, &format!("exec {name}"));
        let output = fixture.run(NVM, &["run", "18", "x"], name);
        assert_refused(&output, name, 1, &format!("run {name}"));
        let output = fixture.run(NVM_EXEC, &CHILD, name);
        assert_refused(&output, name, 127, &format!("nvm-exec {name}"));
    }
}
