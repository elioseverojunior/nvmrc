//! `nvm exec` under a policy: the gate runs before the `Running ...` line and
//! before any child is planned.

use super::*;

fn exec_under(flags: &[&str], path: &str, process: &FakeProcess, args: &[&str]) -> Output {
    let (fs, env) = (lab(), env_on(path));
    let context = Context::new(&fs, &env)
        .with_process(process)
        .with_policy_flags(policy_flags(flags));
    run(&context, &strings(args)).expect("nvm exec runs")
}

#[test]
fn exec_with_a_disallowed_version_runs_nothing() {
    let args = ["18", "node", "-v"];
    let output = exec_under(&["--min-node", "22"], BASE_PATH, &npm_versions(), &args);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(output.spawn, None);
    assert_eq!(output.stdout, "");
    let header = "node v18.20.4 (from the command line) is not allowed by policy:";
    assert!(output.stderr.contains(header), "{}", output.stderr);
}

#[test]
fn exec_with_an_allowed_version_runs_it() {
    let args = ["18", "node", "-v"];
    let output = exec_under(&["--min-node", "18"], BASE_PATH, &npm_versions(), &args);
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.spawn.is_some());
}

#[test]
fn exec_system_checks_the_system_node_version() {
    let process = npm_versions().with_run("/sys/bin/node", "--version", true, "v18.0.0\n");
    let path = "/sys/bin:/usr/bin:/bin";
    let args = ["system", "node", "-v"];
    let output = exec_under(&["--min-node", "20"], path, &process, &args);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(output.spawn, None);
}
