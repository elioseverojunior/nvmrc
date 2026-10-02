//! End-to-end: the real binaries against a real temporary `$NVM_DIR`.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(binary: &str, nvm_dir: &Path, path: &OsStr, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", path)
        .output()
        .expect("run the binary")
}

fn nvm(nvm_dir: &Path, args: &[&str]) -> Output {
    run(
        env!("CARGO_BIN_EXE_nvm"),
        nvm_dir,
        OsStr::new("/nonexistent"),
        args,
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn install(nvm_dir: &Path, version_directory: &str) {
    fs::create_dir_all(nvm_dir.join(version_directory).join("bin")).unwrap();
    fs::write(nvm_dir.join(version_directory).join("bin/node"), "").unwrap();
}

#[test]
fn alias_which_and_unalias_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");

    let created = nvm(dir.path(), &["alias", "work", "20"]);
    assert!(created.status.success());
    assert_eq!(stdout(&created), "work -> 20 (-> v20.1.0 *)\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("alias/work")).unwrap(),
        "20\n"
    );

    let which = nvm(dir.path(), &["which", "work"]);
    let expected = dir.path().join("versions/node/v20.1.0/bin/node");
    assert_eq!(stdout(&which), format!("{}\n", expected.display()));

    let removed = nvm(dir.path(), &["unalias", "work"]);
    assert!(stdout(&removed).starts_with("Deleted alias work - restore it with"));
    assert!(!dir.path().join("alias/work").exists());

    let again = nvm(dir.path(), &["unalias", "work"]);
    assert!(again.status.success());
    assert_eq!(stderr(&again), "Alias work doesn't exist!\n");
}

#[test]
fn alias_to_a_missing_version_warns_but_creates_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let created = nvm(dir.path(), &["alias", "test", "v0.1.2"]);
    assert!(created.status.success());
    assert_eq!(stdout(&created), "test -> v0.1.2 (-> N/A)\n");
    assert_eq!(
        stderr(&created),
        "! WARNING: Version 'v0.1.2' does not exist.\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("alias/test")).unwrap(),
        "v0.1.2\n"
    );
}

#[test]
fn which_of_a_missing_version_exits_1_and_an_alias_loop_exits_8() {
    let dir = tempfile::tempdir().unwrap();
    let missing = nvm(dir.path(), &["which", "16"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).starts_with("N/A: version \"v16\" is not yet installed."));

    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/loop"), "loop\n").unwrap();
    let looped = nvm(dir.path(), &["which", "loop"]);
    assert_eq!(looped.status.code(), Some(8));
    assert_eq!(
        stderr(&looped),
        "The alias \"loop\" leads to an infinite loop. Aborting.\n"
    );
}

#[test]
fn usage_errors_exit_127() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(nvm(dir.path(), &["which"]).status.code(), Some(127));
    assert_eq!(nvm(dir.path(), &["unalias"]).status.code(), Some(127));
    assert_eq!(nvm(dir.path(), &["bogus"]).status.code(), Some(127));
}

#[test]
fn current_reports_none_system_or_an_nvm_version() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");
    let system = tempfile::tempdir().unwrap();
    fs::write(system.path().join("node"), "").unwrap();
    let bin = env!("CARGO_BIN_EXE_nvm");
    let nvm_bin = dir.path().join("versions/node/v20.1.0/bin");

    let none = run(bin, dir.path(), OsStr::new("/nonexistent"), &["current"]);
    assert_eq!(stdout(&none), "none\n");
    let system_path = std::env::join_paths([system.path()]).unwrap();
    let system_run = run(bin, dir.path(), &system_path, &["current"]);
    assert_eq!(stdout(&system_run), "system\n");
    let managed_path = std::env::join_paths([nvm_bin.as_path(), system.path()]).unwrap();
    let managed = run(bin, dir.path(), &managed_path, &["current"]);
    assert_eq!(stdout(&managed), "v20.1.0\n");
}

#[test]
fn both_binaries_behave_the_same() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");
    let path = OsStr::new("/nonexistent");
    let from_nvm = run(
        env!("CARGO_BIN_EXE_nvm"),
        dir.path(),
        path,
        &["version", "20"],
    );
    let from_nvmrc = run(
        env!("CARGO_BIN_EXE_nvmrc"),
        dir.path(),
        path,
        &["version", "20"],
    );
    assert_eq!(stdout(&from_nvm), "v20.1.0\n");
    assert_eq!(from_nvm.stdout, from_nvmrc.stdout);
}

/// Linux only: macOS file systems reject names that are not valid UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_nvm_dir_is_honoured() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let parent = tempfile::tempdir().unwrap();
    let nvm_dir = parent.path().join(OsString::from_vec(b"nvm-\xff".to_vec()));
    install(&nvm_dir, "versions/node/v20.1.0");
    let output = nvm(&nvm_dir, &["version", "20"]);
    assert_eq!(stdout(&output), "v20.1.0\n");
}
