//! `run_and_target`: the switch `nvm use` made, for the callers that run a
//! program in it (`nvm-exec`).

use std::path::Path;

use super::{Target, run_and_target};
use crate::commands::Output;
use crate::commands::exec::tests::{
    BASE_PATH, N18, env_on, lab, npm_versions, policy_flags, strings,
};
use crate::context::Context;
use crate::domain::policy::DEFAULT_INDEX;
use crate::error::NvmExitCode;
use crate::fakes::{FakeHttp, FakeProcess};
use crate::ports::FileSystem;

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

fn switch_under(flags: &[&str], path: &str, process: &FakeProcess, args: &[&str]) -> Output {
    let (fs, env) = (lab(), env_on(path));
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_process(process)
        .with_http(&http)
        .with_policy_flags(policy_flags(flags));
    run_and_target(&context, &strings(args)).unwrap().0
}

#[test]
fn use_of_a_disallowed_installed_version_stops_without_switching() {
    let output = switch_under(&["--min-node", "22"], BASE_PATH, &npm_versions(), &["18"]);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.script.is_empty());
    assert!(
        output.stderr.starts_with("nvm: node v18.20.4 "),
        "{}",
        output.stderr
    );
}

#[test]
fn use_of_a_vulnerable_installed_version_stops_without_switching() {
    let index = r#"{"1": {"cve": ["CVE-2024-27983"], "vulnerable": "18.x",
                         "patched": "^18.20.5", "severity": "high"}}"#;
    let (fs, env, process) = (lab(), env_on(BASE_PATH), npm_versions());
    let http = FakeHttp::default().with_body(DEFAULT_INDEX, index);
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_http(&http)
        .with_policy_flags(policy_flags(&["--fail-on-severity", "high"]));
    let output = run_and_target(&context, &strings(&["18"])).unwrap().0;
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.script.is_empty());
    assert!(
        output
            .stderr
            .contains("  - CVE-2024-27983 (high); the limit is high (from --fail-on-severity)"),
        "{}",
        output.stderr
    );
}

#[test]
fn use_of_an_allowed_version_under_a_policy_switches() {
    let output = switch_under(&["--min-node", "18"], BASE_PATH, &npm_versions(), &["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.script.is_empty());
}

#[test]
fn use_system_checks_the_system_node_version() {
    let process = npm_versions().with_run("/sys/bin/node", "--version", true, "v18.0.0\n");
    let output = switch_under(
        &["--min-node", "20"],
        "/sys/bin:/usr/bin:/bin",
        &process,
        &["system"],
    );
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.script.is_empty());
    assert!(
        output.stderr.contains("node v18.0.0 (system)"),
        "{}",
        output.stderr
    );
}

#[test]
fn use_system_with_an_unreadable_version_cannot_be_verified() {
    let output = switch_under(
        &["--min-node", "20"],
        "/sys/bin:/usr/bin:/bin",
        &npm_versions(),
        &["system"],
    );
    assert_eq!(output.status, NvmExitCode::PolicyUnverifiable);
    assert!(output.script.is_empty());
}

#[test]
fn use_system_without_a_policy_never_asks_the_system_node() {
    let output = switch_under(&[], "/sys/bin:/usr/bin:/bin", &npm_versions(), &["system"]);
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn use_from_an_nvmrc_names_the_file_in_the_message() {
    let (fs, env, process) = (
        lab().with_file("/proj/.nvmrc", "20\n"),
        env_on(BASE_PATH),
        npm_versions(),
    );
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_http(&http)
        .with_policy_flags(policy_flags(&["--min-node", "22"]));
    let output = run_and_target(&context, &[]).unwrap().0;
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    let header = "node v20.11.1 (from /proj/.nvmrc) is not allowed by policy:";
    assert!(output.stderr.contains(header), "{}", output.stderr);
}

#[test]
fn an_unresolvable_nvm_dir_is_a_warning_on_an_automatic_path() {
    use crate::commands::policy::enforce::Gate;
    use crate::commands::policy::gate::gate_system;
    use crate::commands::transcript::Transcript;
    use crate::error::CliError;
    let (fs, env) = (lab(), crate::fakes::FakeEnv::default());
    let flags = policy_flags(&["--min-node", "20"]);
    let base = Context::new(&fs, &env).with_policy_flags(flags);
    let explicit = gate_system(&base, &mut Transcript::default());
    assert!(
        matches!(explicit, Err(CliError::NvmDirUnresolved)),
        "{explicit:?}"
    );
    let mut transcript = Transcript::default();
    let automatic = gate_system(&base.clone().advisory(), &mut transcript);
    assert!(matches!(automatic, Ok(Gate::Pass)), "{automatic:?}");
    let stderr = transcript.finish(NvmExitCode::Success).stderr;
    assert!(stderr.starts_with("nvm: warning:"), "{stderr}");
}

fn save_under(flags: &[&str], args: &[&str]) -> (Output, Option<String>) {
    let (fs, env, process) = (lab(), env_on(BASE_PATH), npm_versions());
    let fs = fs.with_file("/w/.nvmrc", "v22.11.0\n");
    let env = env.with_var("PWD", "/w");
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_http(&http)
        .with_policy_flags(policy_flags(flags));
    let output = run_and_target(&context, &strings(args)).unwrap().0;
    (output, fs.read_to_string(Path::new("/w/.nvmrc")).ok())
}

#[test]
fn a_refused_use_save_leaves_nvmrc_untouched() {
    let (output, saved) = save_under(&["--min-node", "22"], &["--save", "20"]);
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(saved.as_deref(), Some("v22.11.0\n"));
    assert!(
        !output.stdout.contains("Wrote version"),
        "{}",
        output.stdout
    );
}

#[test]
fn an_allowed_use_save_still_writes_nvmrc() {
    let (output, saved) = save_under(&["--min-node", "18"], &["--save", "20"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(saved.as_deref(), Some("v20.11.1\n"));
    assert!(
        output
            .stdout
            .contains("Wrote version number (v20.11.1) to .nvmrc")
    );
}

#[test]
fn legacy_use_save_keeps_its_order() {
    let (output, saved) = save_under(&[], &["--save", "20"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(saved.as_deref(), Some("v20.11.1\n"));
    assert!(
        output
            .stdout
            .starts_with("Wrote version number (v20.11.1) to .nvmrc")
    );
}
