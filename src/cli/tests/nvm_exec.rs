//! `nvm-exec` through the CLI: the command is spawned after the messages,
//! and a program that is not there is reported under the name `nvm-exec`.

use super::*;
use crate::fakes::FakeProcess;

fn nvm_exec_with(command: &[&str], process: &FakeProcess) -> (u8, String, String) {
    let fs = FakeFileSystem::default().with_executable("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NODE_VERSION", "20")
        .with_var("PATH", "/usr/bin");
    let context = Context::new(&fs, &env).with_process(process);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_nvm_exec(command, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn the_command_takes_the_process_over_and_its_status_is_the_status() {
    let process = FakeProcess::default().with_spawn("node", "a", 9);
    let (code, out, err) = nvm_exec_with(&["node", "a"], &process);
    assert_eq!((code, out.as_str(), err.as_str()), (9, "", ""));
    assert_eq!(process.handed_over().len(), 1);
    assert!(process.spawned().is_empty());
}

#[test]
fn a_command_that_is_not_found_is_127_named_nvm_exec() {
    let (code, _, err) = nvm_exec_with(&["nope"], &FakeProcess::default());
    assert_eq!((code, err.as_str()), (127, "nvm-exec: nope: not found\n"));
}

#[test]
fn no_command_is_success() {
    let (code, out, err) = nvm_exec_with(&[], &FakeProcess::default());
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
}
