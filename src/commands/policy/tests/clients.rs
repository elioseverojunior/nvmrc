//! Which client a policy fetch goes through: `NVM_AUTH_HEADER` rides on the
//! regular one, so the registry and the built-in vulnerability index are read
//! with the third-party one instead.

use crate::commands::policy::enforce::{Gate, Origin, Requested, enforce_node};
use crate::commands::project_pins::ProjectPins;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::Pins;
use crate::domain::policy::DEFAULT_INDEX;
use crate::domain::timestamp::days_from_iso_date;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeHttp};

const CONFIGURED_INDEX: &str = "https://sec.test/vuln.json";
const PACKUMENT_URL: &str = "https://registry.npmjs.org/pnpm";

fn requested(pins: Pins) -> Requested {
    Requested {
        version: "v22.11.0".parse().unwrap(),
        origin: Origin::Argument,
        offline: false,
        pins: ProjectPins {
            nvmrc: Some("/w/.nvmrc".into()),
            pins,
        },
    }
}

/// A working directory `/w` holding `policy` as its repository policy.
fn world(policy: &str) -> (FakeFileSystem, FakeEnv) {
    let fs = FakeFileSystem::default()
        .with_dir("/w")
        .with_file("/w/.nvmrc-policy.json", policy);
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    (fs, env)
}

fn run(context: &Context<'_>, request: &Requested) -> Result<Gate, CliError> {
    let mut transcript = Transcript::default();
    let gate = enforce_node(context, request, &mut transcript);
    let _ = transcript.finish(NvmExitCode::Success);
    gate
}

#[test]
fn the_default_index_is_read_with_the_third_party_client() {
    let (fs, env) = world(r#"{ "schemaVersion": 1, "cve": { "failOnSeverity": "high" } }"#);
    let regular = FakeHttp::default();
    let third_party = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_http(&regular)
        .with_third_party_http(&third_party, &third_party);
    let gate = run(&context, &requested(Pins::default()));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    assert_eq!(third_party.requests(), [DEFAULT_INDEX]);
    assert!(regular.requests().is_empty());
}

#[test]
fn a_configured_index_is_read_with_the_regular_client() {
    let policy =
        format!(r#"{{ "schemaVersion": 1, "cve": {{ "source": "{CONFIGURED_INDEX}" }} }}"#);
    let (fs, env) = world(&policy);
    let regular = FakeHttp::default().with_body(CONFIGURED_INDEX, "{}");
    let third_party = FakeHttp::default();
    let context = Context::new(&fs, &env)
        .with_http(&regular)
        .with_third_party_http(&third_party, &third_party);
    let gate = run(&context, &requested(Pins::default()));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    assert_eq!(regular.requests(), [CONFIGURED_INDEX]);
    assert!(third_party.requests().is_empty());
}

#[test]
fn an_age_rule_reads_the_registry_with_the_third_party_client() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimumReleaseAgeDays": 7 } } }"#;
    let (fs, env) = world(policy);
    let packument = r#"{ "time": { "9.12.0": "2026-10-01T08:00:00.000Z" } }"#;
    let regular = FakeHttp::default();
    let third_party = FakeHttp::default()
        .with_empty_advisories()
        .with_body(PACKUMENT_URL, packument);
    let clock = FakeClock::at(days_from_iso_date("2026-10-17").unwrap() * 86_400);
    let context = Context::new(&fs, &env)
        .with_http(&regular)
        .with_third_party_http(&third_party, &third_party)
        .with_clock(&clock);
    let pins = Pins {
        pnpm: Some("v9.12.0".parse().unwrap()),
        ..Pins::default()
    };
    let gate = run(&context, &requested(pins));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    assert_eq!(third_party.requests(), [DEFAULT_INDEX, PACKUMENT_URL]);
    assert!(regular.requests().is_empty());
}
