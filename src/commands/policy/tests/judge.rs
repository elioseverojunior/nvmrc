//! The single judge of "allowed": rules plus the CVE check, index read once.

use crate::commands::policy::judge::Judge;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::fixtures::advisories_json;
use crate::domain::policy::{DEFAULT_INDEX, ReleaseInfo, Violation};
use crate::domain::tool::Tool;
use crate::error::PolicyError;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const AFFECTED: &str = "v22.20.0";
const EMPTY_POLICY: &str = r#"{ "schemaVersion": 1 }"#;

fn release() -> ReleaseInfo {
    ReleaseInfo {
        lts: None,
        published_day: None,
    }
}

fn policy_world(policy: &str) -> (FakeFileSystem, FakeEnv) {
    (
        FakeFileSystem::default().with_file("/p.json", policy),
        FakeEnv::default().with_var("NVMRC_POLICY", "/p.json"),
    )
}

fn violations(
    judge: &mut Judge,
    context: &Context<'_>,
    tool: Tool,
    version: &str,
) -> Result<Vec<Violation>, PolicyError> {
    judge.violations(context, tool, &version.parse().unwrap(), &release())
}

fn loaded(context: &Context<'_>) -> Judge {
    Judge::load(context, &mut Transcript::default()).unwrap()
}

#[test]
fn legacy_mode_allows_everything() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        FakeEnv::default(),
        FakeHttp::default(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    let found = violations(&mut judge, &context, Tool::Node, "v1.0.0").unwrap();
    assert!(found.is_empty());
    assert!(http.requests().is_empty());
}

#[test]
fn a_denied_version_is_not_allowed() {
    let (fs, env) =
        policy_world(r#"{ "schemaVersion": 1, "tools": { "node": { "deny": ["20.12.0"] } } }"#);
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    let denied = violations(&mut judge, &context, Tool::Node, "v20.12.0").unwrap();
    let other = violations(&mut judge, &context, Tool::Node, "v20.12.1").unwrap();
    assert!(!denied.is_empty());
    assert!(other.is_empty());
}

#[test]
fn a_node_cve_above_the_limit_is_not_allowed() {
    let (fs, env) = policy_world(EMPTY_POLICY);
    let env = env.with_var("NVMRC_FAIL_ON_SEVERITY", "high");
    let http = FakeHttp::default().with_body(DEFAULT_INDEX, advisories_json());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    let found = violations(&mut judge, &context, Tool::Node, AFFECTED).unwrap();
    assert!(matches!(found.as_slice(), [Violation::Vulnerable { .. }]));
}

#[test]
fn the_index_is_read_once_for_many_node_versions() {
    let (fs, env) = policy_world(EMPTY_POLICY);
    let http = FakeHttp::default().with_body(DEFAULT_INDEX, advisories_json());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    for version in ["v22.20.0", "v22.19.0", "v20.1.0"] {
        violations(&mut judge, &context, Tool::Node, version).unwrap();
    }
    assert_eq!(http.requests(), [DEFAULT_INDEX]);
}

#[test]
fn a_package_manager_minimum_applies() {
    let (fs, env) =
        policy_world(r#"{ "schemaVersion": 1, "tools": { "npm": { "minimum": "10.2.4" } } }"#);
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    let old = violations(&mut judge, &context, Tool::Npm, "v10.0.0").unwrap();
    let new = violations(&mut judge, &context, Tool::Npm, "v10.9.0").unwrap();
    assert!(!old.is_empty());
    assert!(new.is_empty());
    assert!(http.requests().is_empty());
}

#[test]
fn an_unreadable_index_is_a_failure_to_know() {
    let (fs, env) = policy_world(EMPTY_POLICY);
    let http = FakeHttp::default().with_body(DEFAULT_INDEX, "not json");
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let mut judge = loaded(&context);
    let error = violations(&mut judge, &context, Tool::Node, AFFECTED).unwrap_err();
    assert!(matches!(error, PolicyError::IndexInvalid { .. }));
}
