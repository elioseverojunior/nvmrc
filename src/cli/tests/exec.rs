//! `nvm exec` and `nvm run` through the argument parser: every argument
//! after the subcommand is theirs, help words included.

use super::*;
use crate::fakes::FakeProcess;

fn run_with(args: &[&str], process: &FakeProcess) -> (u8, String, String) {
    let fs = FakeFileSystem::default().with_executable("/n/versions/node/v18.20.4/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let context = Context::new(&fs, &env).with_process(process);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn exec_passes_help_words_and_options_to_the_command_and_its_status_back() {
    let process = FakeProcess::default().with_spawn("node", "-h --help help --x", 3);
    let args = ["nvm", "exec", "18", "node", "-h", "--help", "help", "--x"];
    let result = run_with(&args, &process);
    assert_eq!(result, (3, "Running node v18.20.4\n".into(), String::new()));
}

#[test]
fn exec_options_before_the_version_are_its_own() {
    let process = FakeProcess::default().with_spawn("node", "a", 0);
    let result = run_with(&["nvm", "exec", "--silent", "18", "node", "a"], &process);
    assert_eq!(result, (0, String::new(), String::new()));
}

#[test]
fn exec_rejects_an_unknown_option_with_55() {
    let result = run_with(&["nvm", "exec", "--bogus", "18"], &FakeProcess::default());
    let expected = (
        55,
        String::new(),
        "Unsupported option \"--bogus\".\n".into(),
    );
    assert_eq!(result, expected);
}

#[test]
fn run_hands_its_arguments_to_node() {
    let process = FakeProcess::default().with_spawn("node", "--help -- x", 0);
    let args = ["nvm", "run", "--silent", "18", "--help", "--", "x"];
    assert_eq!(run_with(&args, &process), (0, String::new(), String::new()));
    assert_eq!(process.spawned().len(), 1);
}
