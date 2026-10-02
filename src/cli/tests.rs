use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

fn run_cli(args: &[&str]) -> (u8, String, String) {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn version_prints_the_match_and_exits_zero() {
    assert_eq!(
        run_cli(&["nvm", "version", "20"]),
        (0, "v20.1.0\n".into(), String::new())
    );
}

#[test]
fn which_prints_the_binary_path() {
    assert_eq!(
        run_cli(&["nvm", "which", "20"]),
        (
            0,
            "/n/versions/node/v20.1.0/bin/node\n".into(),
            String::new()
        )
    );
}

#[test]
fn which_of_a_missing_version_fails_on_stderr_with_exit_1() {
    let (code, out, err) = run_cli(&["nvm", "which", "16"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.starts_with("N/A: version \"v16\" is not yet installed."));
}

#[test]
fn which_without_an_argument_is_a_usage_error_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "which"]);
    assert_eq!(code, 127);
    assert!(out.is_empty() && err.starts_with("Usage: nvm which"));
}

#[test]
fn unalias_removes_the_alias_and_says_how_to_restore_it() {
    let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "unalias", "work"];
    let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
    assert_eq!(code, 0);
    let printed = String::from_utf8(out).unwrap();
    assert!(printed.starts_with("Deleted alias work - restore it"));
    assert!(!fs.is_file(std::path::Path::new("/n/alias/work")));
}

#[test]
fn alias_creates_the_file_and_prints_the_formatted_line() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "alias", "work", "20"];
    let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
    assert_eq!(
        (code, out, err),
        (0, b"work -> 20 (-> v20.1.0 *)\n".to_vec(), Vec::new())
    );
    let stored = fs.read_to_string(std::path::Path::new("/n/alias/work"));
    assert_eq!(stored.unwrap(), "20\n");
}

#[test]
fn unalias_without_a_name_is_a_usage_error_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "unalias"]);
    assert_eq!(code, 127);
    assert!(out.is_empty() && err.starts_with("Usage: nvm unalias <name>"));
}

#[test]
fn current_prints_the_active_version() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/n/versions/node/v20.1.0/bin");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(
        ["nvm", "current"],
        &Context { fs: &fs, env: &env },
        &mut out,
        &mut err,
    );
    assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
}

#[test]
fn version_without_an_argument_means_current() {
    assert_eq!(
        run_cli(&["nvm", "version"]),
        (0, "none\n".into(), String::new())
    );
}

#[test]
fn version_reports_not_installed_on_stdout_with_exit_3() {
    assert_eq!(
        run_cli(&["nvm", "version", "16"]),
        (3, "N/A\n".into(), String::new())
    );
}

#[test]
fn version_of_a_non_version_name_is_na_on_stdout_with_exit_3() {
    assert_eq!(
        run_cli(&["nvm", "version", "foo"]),
        (3, "N/A\n".into(), String::new())
    );
}

#[test]
fn an_alias_loop_is_reported_on_stderr_with_exit_8() {
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/a", "b")
        .with_file("/n/alias/b", "a");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "version", "a"];
    let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
    assert_eq!(code, 8);
    assert!(out.is_empty() && !err.is_empty());
}

struct BrokenPipe;

impl Write for BrokenPipe {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

#[test]
fn a_failed_stdout_write_exits_1() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let mut err = Vec::new();
    let args = ["nvm", "version", "20"];
    let code = run(
        args,
        &Context { fs: &fs, env: &env },
        &mut BrokenPipe,
        &mut err,
    );
    assert_eq!(code, 1);
}

#[test]
fn a_failed_stderr_write_keeps_the_exit_code() {
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/a", "b")
        .with_file("/n/alias/b", "a");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let mut out = Vec::new();
    let args = ["nvm", "version", "a"];
    let code = run(
        args,
        &Context { fs: &fs, env: &env },
        &mut out,
        &mut BrokenPipe,
    );
    assert_eq!(code, 8);
}

#[test]
fn a_usage_error_exits_non_zero_without_stdout() {
    let (code, out, err) = run_cli(&["nvm", "bogus"]);
    assert_ne!(code, 0);
    assert!(out.is_empty() && !err.is_empty());
}
