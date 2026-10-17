use crate::commands::policy::enforce::{Gate, Origin, Requested, enforce_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::fixtures::index_text_dated;
use crate::domain::policy::{DEFAULT_INDEX, PolicyFlags, extract};
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeHttp};
use std::ffi::OsString;

const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
const LTS_POLICY: &str = r#"{ "schemaVersion": 1, "tools": { "node": { "ltsOnly": true } } }"#;

fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

fn requested(version: &str, origin: Origin, offline: bool) -> Requested {
    Requested {
        version: version.parse().unwrap(),
        origin,
        offline,
        pins: crate::commands::project_pins::ProjectPins::default(),
    }
}

fn argument(version: &str) -> Requested {
    requested(version, Origin::Argument, false)
}

fn index() -> String {
    index_text_dated(&[
        ("v23.1.0", "2024-10-24", "-"),
        ("v20.15.1", "2024-07-08", "Iron"),
    ])
}

/// The release index and an empty vulnerability index.
fn served() -> FakeHttp {
    FakeHttp::default()
        .with_body(INDEX_URL, &index())
        .with_empty_advisories()
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

/// A world whose policy file only allows LTS releases.
fn lts_world() -> (FakeEnv, FakeFileSystem) {
    let fs = FakeFileSystem::default().with_file("/p.json", LTS_POLICY);
    (env().with_var("NVMRC_POLICY", "/p.json"), fs)
}

/// What the gate said and what it printed on stderr.
fn run(context: &Context<'_>, request: &Requested) -> (Result<Gate, CliError>, String) {
    let mut transcript = Transcript::default();
    let gate = enforce_node(context, request, &mut transcript);
    (gate, transcript.finish(NvmExitCode::Success).stderr)
}

fn min_flags() -> PolicyFlags {
    flags(&["--min-node", "20.11.1"])
}

#[test]
fn legacy_mode_always_passes() {
    let (env, fs) = (env(), FakeFileSystem::default());
    let (gate, stderr) = run(&Context::new(&fs, &env), &argument("v16.0.0"));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(stderr.is_empty());
}

#[test]
fn a_violation_stops_with_7_and_names_everything() {
    let (env, fs, http) = (env(), FakeFileSystem::default(), served());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_policy_flags(min_flags());
    let origin = Origin::Nvmrc("/w/app/.nvmrc".into());
    let (gate, stderr) = run(&context, &requested("v18.19.0", origin, false));
    assert!(matches!(
        gate,
        Ok(Gate::Stop(NvmExitCode::BelowVersionFloor))
    ));
    assert_eq!(
        stderr,
        [
            "nvm: node v18.19.0 (from /w/app/.nvmrc) is not allowed by policy:",
            "  - below the minimum v20.11.1 (from --min-node)",
            "Raise /w/app/.nvmrc to an allowed version.",
        ]
        .join("\n")
    );
}

#[test]
fn an_advisory_context_warns_and_passes() {
    let (env, fs, http) = (env(), FakeFileSystem::default(), served());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_policy_flags(min_flags())
        .advisory();
    let (gate, stderr) = run(&context, &argument("v18.19.0"));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert_eq!(
        stderr,
        [
            "nvm: warning: node v18.19.0 (from the command line) is not allowed by policy:",
            "  - below the minimum v20.11.1 (from --min-node)",
            "Use an allowed version.",
        ]
        .join("\n")
    );
}

#[test]
fn rules_needing_release_info_fetch_the_index_once() {
    let (env, fs) = lts_world();
    let http = served();
    let clock = FakeClock::at(1_791_110_040);
    let context = Context::new(&fs, &env).with_http(&http).with_clock(&clock);
    let (gate, stderr) = run(&context, &argument("v23.1.0"));
    assert!(matches!(gate, Ok(Gate::Stop(_))));
    assert!(stderr.contains("  - not an LTS release (from policy /p.json)"));
    assert_eq!(http.requests(), [INDEX_URL, DEFAULT_INDEX]);
}

#[test]
fn an_lts_release_passes_the_lts_rule() {
    let (env, fs) = lts_world();
    let http = served();
    let context = Context::new(&fs, &env).with_http(&http);
    assert!(matches!(
        run(&context, &argument("v20.15.1")).0,
        Ok(Gate::Pass)
    ));
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let (env, fs) = lts_world();
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http);
    let error = run(&context, &argument("v20.15.1")).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn a_version_missing_from_the_index_is_a_failure_to_know() {
    let (env, fs) = lts_world();
    let http = FakeHttp::default().with_body(INDEX_URL, &index());
    let context = Context::new(&fs, &env).with_http(&http);
    let error = run(&context, &argument("v21.0.0")).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn offline_with_a_release_rule_is_a_failure_to_know() {
    let (env, fs) = lts_world();
    let http = FakeHttp::default().with_body(INDEX_URL, &index());
    let context = Context::new(&fs, &env).with_http(&http);
    let request = requested("v20.15.1", Origin::Argument, true);
    assert_eq!(run(&context, &request).0.unwrap_err().exit_code().code(), 9);
    assert!(http.requests().is_empty());
}

#[test]
fn an_advisory_failure_to_know_warns_and_passes() {
    let (env, fs) = lts_world();
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http).advisory();
    let (gate, stderr) = run(&context, &argument("v20.15.1"));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(stderr.starts_with("nvm: warning: cannot check the policy:"));
    assert!(!stderr.contains("nvm: warning: nvm:"));
}

#[test]
fn an_advisory_index_fetch_goes_through_the_quick_client() {
    let (env, fs) = lts_world();
    let regular = FakeHttp::default();
    let quick = served();
    let context = Context::new(&fs, &env)
        .with_http(&regular)
        .with_quick_http(&quick)
        .advisory();
    let (gate, stderr) = run(&context, &argument("v23.1.0"));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(stderr.contains("not an LTS release"), "{stderr}");
    assert_eq!(quick.requests(), [INDEX_URL, DEFAULT_INDEX]);
    assert!(regular.requests().is_empty());
}
