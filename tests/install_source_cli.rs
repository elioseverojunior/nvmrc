//! End-to-end: building a version from source (`-s`, the fallback after a
//! failed binary, `-b`, `-j`) with a fake `./configure` in the source archive
//! and a fake `make` on `PATH`.
#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use common::{Mirror, nvm_with, stderr, stdout};

/// Remembers the prefix it was given, for the fake `make`.
const CONFIGURE: &str = r#"#!/bin/sh
echo "configure args: $*" >&2
PREFIX=
for arg in "$@"; do case "$arg" in --prefix=*) PREFIX="${arg#--prefix=}" ;; esac; done
printf 'PREFIX=%s\n' "$PREFIX" > Makefile
"#;

/// Logs what it was asked, and `install` puts a `node` in the prefix.
const MAKE: &str = r#"#!/bin/sh
echo "make $*" >> "$MAKE_LOG"
case " $* " in
  *" install "*)
    PREFIX=$(sed -n 's/^PREFIX=//p' Makefile)
    mkdir -p "$PREFIX/bin"
    printf '#!/bin/sh\necho built-from-source\n' > "$PREFIX/bin/node"
    chmod +x "$PREFIX/bin/node" ;;
esac
"#;

struct Setup {
    home: tempfile::TempDir,
    tools: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        let tools = tempfile::tempdir().unwrap();
        let make = tools.path().join("make");
        fs::write(&make, MAKE).unwrap();
        fs::set_permissions(&make, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            home: tempfile::tempdir().unwrap(),
            tools,
        }
    }

    fn log(&self) -> String {
        fs::read_to_string(self.tools.path().join("make.log")).unwrap_or_default()
    }

    fn run(&self, mirror: &str, args: &[&str]) -> std::process::Output {
        let path = format!("{}:/usr/bin:/bin", self.tools.path().display());
        let log = self.tools.path().join("make.log");
        let extra = [("MAKE_LOG", log.to_str().unwrap())];
        nvm_with(self.home.path(), mirror, args, &path, &extra)
    }
}

fn source_only() -> String {
    Mirror::new()
        .source("v20.10.0", &[("configure", CONFIGURE, 0o755)])
        .serve()
}

fn node(home: &Path) -> String {
    let node = home.join("versions/node/v20.10.0/bin/node");
    let ran = Command::new(node).output().unwrap();
    String::from_utf8_lossy(&ran.stdout).into_owned()
}

#[test]
fn s_builds_from_source_and_the_result_runs() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-j", "3", "-s", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let prefix = setup.home.path().join("versions/node/v20.10.0");
    let out = stdout(&output);
    assert!(out.starts_with("number of `make` jobs: 3\n"));
    assert!(out.contains(&format!("$>./configure --prefix={} <", prefix.display())));
    // macOS and the BSDs add `CC=cc CXX=c++`.
    let log = setup.log();
    let calls: Vec<&str> = log.lines().collect();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with("make -j 3") && !calls[0].ends_with("install"));
    assert!(calls[1].starts_with("make -j 3") && calls[1].ends_with(" install"));
    assert_eq!(node(setup.home.path()), "built-from-source\n");
}

#[test]
fn a_failed_binary_falls_back_to_source() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-j", "2", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).contains("Binary download failed, trying source."));
    assert_eq!(node(setup.home.path()), "built-from-source\n");
}

#[test]
fn b_does_not_build_after_a_failed_binary() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-b", "20"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
    assert_eq!(setup.log(), "");
}

#[test]
fn the_words_after_the_version_reach_configure() {
    let setup = Setup::new();
    let output = setup.run(
        &source_only(),
        &["install", "-j", "2", "-s", "20", "--ninja"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Additional options while compiling:  --ninja"));
    assert!(stderr(&output).contains("--ninja"));
}

#[test]
fn a_failing_make_is_status_1_and_installs_nothing() {
    let setup = Setup::new();
    let make = setup.tools.path().join("make");
    fs::write(&make, "#!/bin/sh\necho no good >&2\nexit 2\n").unwrap();
    let output = setup.run(&source_only(), &["install", "-j", "2", "-s", "20"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).ends_with("no good\nnvm: install v20.10.0 failed!\n"));
    assert!(!setup.home.path().join("versions").exists());
}

#[test]
fn the_third_party_hook_installs_in_place_of_nvm_and_its_status_is_passed_on() {
    let setup = Setup::new();
    let hook = setup.tools.path().join("hook");
    fs::write(&hook, "#!/bin/sh\necho \"hook: $*\"\nexit 9\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", setup.tools.path().display());
    let extra = [("NVM_INSTALL_THIRD_PARTY_HOOK", hook.to_str().unwrap())];
    let output = nvm_with(
        setup.home.path(),
        &source_only(),
        &["install", "20"],
        &path,
        &extra,
    );
    assert_eq!(output.status.code(), Some(9));
    assert!(stdout(&output).starts_with("hook: v20.10.0 node std binary "));
    assert!(stderr(&output).ends_with(
        "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***\n"
    ));
}
