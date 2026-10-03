use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

fn run_cli(args: &[&str]) -> (u8, String, String) {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
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
fn which_without_an_argument_and_without_an_nvmrc_says_so_then_the_usage_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "which"]);
    assert_eq!(code, 127);
    assert!(out.is_empty());
    assert!(err.starts_with("No version provided and no .nvmrc file found\nUsage: nvm which"));
}

#[test]
fn unalias_removes_the_alias_and_says_how_to_restore_it() {
    let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "unalias", "work"];
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
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
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
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
        &Context::new(&fs, &env),
        &mut out,
        &mut err,
    );
    assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
}

#[test]
fn ls_prints_the_rows_then_the_aliases_and_list_is_the_same_command() {
    let rows = "        v20.1.0 *\n";
    let aliases = "iojs -> N/A (default)\n\
                   node -> stable (-> v20.1.0 *) (default)\n\
                   stable -> 20.1 (-> v20.1.0 *) (default)\n\
                   unstable -> N/A (default)\n";
    let expected = (0, format!("{rows}{aliases}"), String::new());
    assert_eq!(run_cli(&["nvm", "ls"]), expected);
    assert_eq!(run_cli(&["nvm", "list"]), expected);
    let without = (0, rows.to_owned(), String::new());
    assert_eq!(run_cli(&["nvm", "ls", "--no-alias"]), without);
    assert_eq!(
        run_cli(&["nvm", "ls", "--no-colors", "--no-alias"]),
        without
    );
}

#[test]
fn unsupported_options_exit_55_with_the_nvm_sh_message() {
    assert_eq!(
        run_cli(&["nvm", "ls", "--bogus"]),
        (
            55,
            String::new(),
            "Unsupported option \"--bogus\".\n".to_owned()
        )
    );
    assert_eq!(
        run_cli(&["nvm", "alias", "--bogus"]),
        (
            55,
            String::new(),
            "Unsupported option \"--bogus\".\n".to_owned()
        )
    );
    let (code, out, err) = run_cli(&["nvm", "ls", "20", "--no-alias"]);
    assert_eq!(code, 55);
    assert!(out.is_empty());
    assert_eq!(
        err,
        "`--no-alias` is not supported when a pattern is provided.\n"
    );
}

#[test]
fn alias_without_arguments_lists_the_aliases() {
    let (code, out, _) = run_cli(&["nvm", "alias"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("iojs -> N/A (default)\n"), "{out}");
}

#[test]
fn ls_of_a_missing_version_prints_na_on_stdout_and_exits_3() {
    assert_eq!(
        run_cli(&["nvm", "ls", "16"]),
        (3, "            N/A\n".to_owned(), String::new())
    );
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
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
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
    let code = run(args, &Context::new(&fs, &env), &mut BrokenPipe, &mut err);
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
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut BrokenPipe);
    assert_eq!(code, 8);
}

#[test]
fn a_usage_error_exits_127_without_stdout() {
    for args in [&["nvm", "bogus"][..], &["nvm", "which", "--silent"]] {
        let (code, out, err) = run_cli(args);
        assert_eq!(code, 127, "{args:?}");
        assert!(out.is_empty() && !err.is_empty(), "{args:?}");
    }
}

#[test]
fn help_and_version_go_to_stdout_with_exit_0() {
    for flag in ["--help", "--version"] {
        let (code, out, err) = run_cli(&["nvm", flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(!out.is_empty() && err.is_empty(), "{flag}");
    }
}

#[test]
fn ls_remote_and_list_remote_print_the_releases_with_the_exit_status() {
    let index = index_text(&[("v20.10.0", "Iron"), ("v20.9.0", "Iron")]);
    let http = FakeHttp::default()
        .with_body("https://nodejs.org/dist/index.tab", &index)
        .with_status("https://iojs.org/dist/index.tab", 404);
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    for command in ["ls-remote", "list-remote"] {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let context = Context::new(&fs, &env).with_http(&http);
        let code = run(["nvm", command, "--lts"], &context, &mut out, &mut err);
        let expected = "        v20.9.0   (LTS: Iron)\n       v20.10.0   (Latest LTS: Iron)\n";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
        assert_eq!(code, 0);
    }
}

#[test]
fn install_and_its_alias_i_without_a_version_are_a_usage_error_with_exit_127() {
    for command in ["install", "i"] {
        let (code, out, err) = run_cli(&["nvm", command]);
        assert_eq!((code, out.as_str()), (127, ""));
        assert!(
            err.starts_with("No version provided and no .nvmrc file found\nUsage: nvm install")
        );
    }
}

#[test]
fn uninstall_needs_one_word_and_a_missing_version_is_only_a_message() {
    let (code, _, err) = run_cli(&["nvm", "uninstall"]);
    assert_eq!(code, 127);
    assert!(err.starts_with("Usage: nvm uninstall <version>"));
    let (code, out, err) = run_cli(&["nvm", "uninstall", "99"]);
    assert_eq!((code, out.as_str()), (0, ""));
    assert_eq!(err, "Version '99' is not installed.\n");
}

mod exec;
mod install;
mod nvm_exec;
mod script;
mod spawn;
