//! `run_and_target`: the switch `nvm use` made, for the callers that run a
//! program in it (`nvm-exec`).

use std::path::Path;

use super::{Target, run_and_target};
use crate::commands::Output;
use crate::commands::exec::tests::{BASE_PATH, N18, env_on, lab, npm_versions, strings};
use crate::context::Context;
use crate::error::NvmExitCode;

fn switch(args: &[&str]) -> (Output, Option<Target>) {
    let (fs, env, process) = (lab(), env_on(BASE_PATH), npm_versions());
    let context = Context::new(&fs, &env).with_process(&process);
    run_and_target(&context, &strings(args)).unwrap()
}

#[test]
fn a_switch_returns_the_target_it_applied() {
    let (output, target) = switch(&["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    match target {
        Some(Target::Installed { directory, .. }) => assert_eq!(directory, Path::new(N18)),
        other => panic!("not the installed v18: {other:?}"),
    }
}

#[test]
fn a_stop_before_the_switch_returns_no_target() {
    let (output, target) = switch(&["99"]);
    assert_ne!(output.status, NvmExitCode::Success);
    assert!(target.is_none());
}
