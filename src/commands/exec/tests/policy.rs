//! `nvm exec` under a policy: the gate runs before the `Running ...` line and
//! before any child is planned.

use super::*;

fn exec_under(flags: &[&str], path: &str, process: &FakeProcess, args: &[&str]) -> Output {
    let (fs, env) = (lab(), env_on(path));
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_process(process)
        .with_http(&http)
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

#[test]
fn exec_system_with_an_unreadable_version_says_so_and_fails_to_know() {
    let process = npm_versions().with_run("/sys/bin/node", "--version", false, "");
    let (fs, env) = (lab(), env_on("/sys/bin:/usr/bin:/bin"));
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_policy_flags(policy_flags(&["--min-node", "20"]));
    let error = run(&context, &strings(&["system", "node", "-v"])).unwrap_err();
    assert_eq!(
        error.to_string(),
        "nvm: cannot check the policy: the version of the system node cannot be read."
    );
    assert_eq!(error.exit_code(), NvmExitCode::PolicyUnverifiable);
}

fn exec_in_under(fs: &FakeFileSystem, path: &str, args: &[&str]) -> Output {
    let (env, process) = (env_on(path), npm_versions());
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(fs, &env)
        .with_process(&process)
        .with_http(&http)
        .with_policy_flags(policy_flags(&["--min-node", "22"]));
    run(&context, &strings(args)).expect("nvm exec runs")
}

#[test]
fn exec_from_an_nvmrc_names_the_file_in_the_message() {
    let fs = lab().with_file("/proj/.nvmrc", "20\n");
    let output = exec_in_under(&fs, BASE_PATH, &["npm", "-v"]);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    let header = "node v20.11.1 (from /proj/.nvmrc) is not allowed by policy:";
    assert!(output.stderr.contains(header), "{}", output.stderr);
    assert!(
        output
            .stderr
            .ends_with("Raise /proj/.nvmrc to an allowed version.")
    );
}

#[test]
fn exec_of_the_active_version_says_so() {
    let output = exec_in_under(&lab(), ACTIVE_18, &[]);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    let header = "node v18.20.4 (the active version) is not allowed by policy:";
    assert!(output.stderr.contains(header), "{}", output.stderr);
}
