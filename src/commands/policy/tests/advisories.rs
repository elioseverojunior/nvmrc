//! The CVE check of the gate: the vulnerability index against the version.

use crate::commands::policy::enforce::{Gate, Origin, Requested, enforce_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::fixtures::advisories_json;
use crate::domain::policy::DEFAULT_INDEX;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const AFFECTED: &str = "v22.20.0";

fn requested(version: &str, offline: bool) -> Requested {
    Requested {
        version: version.parse().unwrap(),
        origin: Origin::Argument,
        offline,
        pins: crate::commands::project_pins::ProjectPins::default(),
    }
}

fn limit_env(limit: &str) -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVMRC_FAIL_ON_SEVERITY", limit)
}

fn served(body: &str) -> FakeHttp {
    FakeHttp::default().with_body(DEFAULT_INDEX, body)
}

/// What the gate said and what it printed on stderr.
fn run(context: &Context<'_>, request: &Requested) -> (Result<Gate, CliError>, String) {
    let mut transcript = Transcript::default();
    let gate = enforce_node(context, request, &mut transcript);
    (gate, transcript.finish(NvmExitCode::Success).stderr)
}

/// The gate under `limit` with the fixture index served at the default URL.
fn gate_under(limit: &str, version: &str) -> (Result<Gate, CliError>, String, Vec<String>) {
    let (env, fs, http) = (
        limit_env(limit),
        FakeFileSystem::default(),
        served(advisories_json()),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let (gate, stderr) = run(&context, &requested(version, false));
    (gate, stderr, http.requests())
}

#[test]
fn a_finding_at_the_limit_stops_with_7() {
    let (gate, stderr, requests) = gate_under("high", AFFECTED);
    assert!(matches!(
        gate,
        Ok(Gate::Stop(NvmExitCode::BelowVersionFloor))
    ));
    assert_eq!(
        stderr,
        [
            "nvm: node v22.20.0 (from the command line) is not allowed by policy:",
            "  - CVE-2026-58043 (high); the limit is high (from NVMRC_FAIL_ON_SEVERITY)",
            "Use an allowed version.",
        ]
        .join("\n")
    );
    assert_eq!(requests, [DEFAULT_INDEX]);
}

#[test]
fn findings_below_the_limit_only_warn() {
    let (gate, stderr, _) = gate_under("critical", AFFECTED);
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert_eq!(
        stderr,
        "nvm: warning: node v22.20.0 has known vulnerabilities below the limit critical: \
         CVE-2026-58043 (high)"
    );
}

#[test]
fn limit_none_reports_and_passes() {
    let (gate, stderr, _) = gate_under("none", AFFECTED);
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(
        stderr.contains("below the limit none: CVE-2026-58043 (high)"),
        "{stderr}"
    );
}

#[test]
fn a_patched_version_passes_quietly() {
    let (gate, stderr, _) = gate_under("low", "v22.23.2");
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn findings_below_the_limit_are_also_named_when_the_gate_stops() {
    let env = limit_env("medium");
    let index = r#"{
      "1": {"cve": ["CVE-A"], "vulnerable": "22.x", "severity": "high"},
      "2": {"cve": ["CVE-B"], "vulnerable": "22.x", "severity": "low"}
    }"#;
    let (fs, http) = (FakeFileSystem::default(), served(index));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let (gate, stderr) = run(&context, &requested(AFFECTED, false));
    assert!(matches!(gate, Ok(Gate::Stop(_))));
    assert_eq!(
        stderr,
        [
            "nvm: node v22.20.0 (from the command line) is not allowed by policy:",
            "  - CVE-A (high); the limit is medium (from NVMRC_FAIL_ON_SEVERITY)",
            "Use an allowed version.",
            "nvm: warning: node v22.20.0 has known vulnerabilities below the limit medium: \
             CVE-B (low)",
        ]
        .join("\n")
    );
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let (env, fs, http) = (
        limit_env("high"),
        FakeFileSystem::default(),
        FakeHttp::default(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let error = run(&context, &requested(AFFECTED, false)).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn an_invalid_index_names_its_location() {
    let (env, fs, http) = (limit_env("high"), FakeFileSystem::default(), served("[]"));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let error = run(&context, &requested(AFFECTED, false)).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(error.to_string().starts_with(&format!(
        "nvm: cannot check the policy: the vulnerability index {DEFAULT_INDEX} is not valid: "
    )));
}

#[test]
fn offline_with_a_remote_index_is_a_failure_to_know() {
    let (env, fs, http) = (
        limit_env("high"),
        FakeFileSystem::default(),
        served(advisories_json()),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let error = run(&context, &requested(AFFECTED, true)).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(error.to_string().contains("offline"), "{error}");
    assert!(http.requests().is_empty());
}

#[test]
fn offline_with_an_uppercase_https_index_is_a_failure_to_know() {
    let policy = r#"{ "schemaVersion": 1, "cve": { "source": "HTTPS://sec/vuln.json" } }"#;
    let fs = FakeFileSystem::default()
        .with_dir("/w")
        .with_file("/w/.nvmrc-policy.json", policy);
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let http = FakeHttp::default().with_body("HTTPS://sec/vuln.json", advisories_json());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let error = run(&context, &requested(AFFECTED, true)).0.unwrap_err();
    assert!(error.to_string().contains("offline"), "{error}");
    assert!(http.requests().is_empty());
}

#[test]
fn offline_with_a_local_index_is_checked() {
    let policy =
        r#"{ "schemaVersion": 1, "cve": { "failOnSeverity": "high", "source": "vuln.json" } }"#;
    let fs = FakeFileSystem::default()
        .with_dir("/w")
        .with_file("/w/.nvmrc-policy.json", policy)
        .with_file("/w/vuln.json", advisories_json());
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let (gate, stderr) = run(&context, &requested(AFFECTED, true));
    assert!(matches!(gate, Ok(Gate::Stop(_))));
    assert!(stderr.contains("  - CVE-2026-58043 (high)"), "{stderr}");
    assert!(http.requests().is_empty());
}

#[test]
fn an_automatic_path_warns_on_a_finding_and_passes() {
    let (env, fs, http) = (
        limit_env("high"),
        FakeFileSystem::default(),
        served(advisories_json()),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .advisory();
    let (gate, stderr) = run(&context, &requested(AFFECTED, false));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(
        stderr.starts_with("nvm: warning: node v22.20.0 (from the command line) is not allowed"),
        "{stderr}"
    );
    assert!(stderr.contains("  - CVE-2026-58043 (high)"), "{stderr}");
}

#[test]
fn legacy_mode_never_fetches_the_index() {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_MIN_VERSION", "18");
    let (fs, http) = (FakeFileSystem::default(), served(advisories_json()));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let (gate, _) = run(&context, &requested(AFFECTED, false));
    assert!(matches!(gate, Ok(Gate::Pass)));
    assert!(http.requests().is_empty());
}

#[test]
fn iojs_versions_skip_the_cve_check() {
    let (gate, stderr, requests) = gate_under("high", "iojs-v3.3.1");
    assert!(matches!(gate, Ok(Gate::Stop(_))));
    assert!(
        stderr.contains("io.js is not covered by policy"),
        "{stderr}"
    );
    assert!(!stderr.contains("CVE"), "{stderr}");
    assert!(requests.is_empty());
}
