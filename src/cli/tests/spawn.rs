//! A command that leaves a program to run (`nvm exec`, `nvm run`): its
//! messages are printed first, then the program runs and its status is the
//! status of `nvm`.

use std::io;

use super::*;
use crate::fakes::FakeProcess;
use crate::ports::Invocation;

fn finish_with(output: &Output, process: &FakeProcess) -> (u8, String, String) {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let context = Context::new(&fs, &env).with_process(process);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = finish(output, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn running(invocation: Invocation) -> Output {
    Output::stdout("Running node v18.20.4")
        .with_stderr("careful")
        .with_spawn(invocation)
}

#[test]
fn the_messages_are_printed_and_the_program_runs_with_its_status_passed_on() {
    let process = FakeProcess::default().with_spawn("node", "a b", 7);
    let invocation = Invocation::new("node").args(&["a", "b"]);
    let result = finish_with(&running(invocation.clone()), &process);
    let expected = (7, "Running node v18.20.4\n".into(), "careful\n".into());
    assert_eq!(result, expected);
    assert_eq!(process.spawned(), [invocation]);
}

#[test]
fn a_program_that_succeeds_gives_zero() {
    let process = FakeProcess::default().with_spawn("node", "", 0);
    let (code, _, _) = finish_with(&running(Invocation::new("node")), &process);
    assert_eq!(code, 0);
}

#[test]
fn a_program_that_is_not_found_is_127_with_a_message() {
    let process = FakeProcess::default();
    let (code, out, err) = finish_with(&running(Invocation::new("nope")), &process);
    assert_eq!((code, out.as_str()), (127, "Running node v18.20.4\n"));
    assert_eq!(err, "careful\nnvm: nope: not found\n");
}

#[test]
fn a_program_that_cannot_start_is_127_with_the_reason() {
    let denied = io::ErrorKind::PermissionDenied;
    let process = FakeProcess::default().with_spawn_failure("node", "", denied);
    let (code, _, err) = finish_with(&running(Invocation::new("node")), &process);
    assert_eq!(code, 127);
    let reason = io::Error::from(denied);
    assert_eq!(err, format!("careful\nnvm: cannot run node: {reason}\n"));
}

#[test]
fn without_a_program_nothing_runs() {
    let process = FakeProcess::default();
    let (code, _, _) = finish_with(&Output::stdout("x"), &process);
    assert_eq!(code, 0);
    assert!(process.spawned().is_empty());
}
