//! The rules of npm, yarn and pnpm: checked on the pinned versions, and on
//! the npm bundled with node when npm has a rule but no pin.

use crate::commands::policy::bundled::bundled_npm;
use crate::commands::policy::enforce::{Gate, Origin, Requested, enforce_node};
use crate::commands::project_pins::ProjectPins;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::fixtures::index_text;
use crate::domain::nvmrc::pins::Pins;
use crate::domain::policy::{DEFAULT_INDEX, PolicyFlags, extract};
use crate::domain::timestamp::days_from_iso_date;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeHttp};
use std::ffi::OsString;

const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
const PACKUMENT_URL: &str = "https://registry.npmjs.org/pnpm";
const AGE_POLICY: &str =
    r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimumReleaseAgeDays": 7 } } }"#;
const PNPM_BLOCK: [&str; 3] = [
    "nvm: pnpm 9.1.0 (from /p/.nvmrc) is not allowed by policy:",
    "  - below the minimum 9.12.0 (from --min-pnpm)",
    "Raise /p/.nvmrc to an allowed version.",
];

fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

fn pinned(pins: Pins) -> ProjectPins {
    ProjectPins {
        nvmrc: Some("/p/.nvmrc".into()),
        pins,
    }
}

fn pnpm(version: &str) -> ProjectPins {
    pinned(Pins {
        pnpm: Some(version.parse().unwrap()),
        ..Pins::default()
    })
}

fn requested(version: &str, pins: ProjectPins) -> Requested {
    Requested {
        version: version.parse().unwrap(),
        origin: Origin::Argument,
        offline: false,
        pins,
    }
}

fn env() -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/p")
}

/// What the gate said and what it printed on stderr.
fn run(context: &Context<'_>, request: &Requested) -> (Result<Gate, CliError>, String) {
    let mut transcript = Transcript::default();
    let gate = enforce_node(context, request, &mut transcript);
    (gate, transcript.finish(NvmExitCode::Success).stderr)
}

fn stopped(gate: &Result<Gate, CliError>) -> bool {
    matches!(gate, Ok(Gate::Stop(NvmExitCode::BelowVersionFloor)))
}

#[test]
fn a_pinned_pnpm_below_the_minimum_exits_7() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        env(),
        FakeHttp::default().with_empty_advisories(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-pnpm", "9.12.0"]));
    let (gate, stderr) = run(&context, &requested("v22.11.0", pnpm("9.1.0")));
    assert!(stopped(&gate), "{gate:?}");
    assert_eq!(stderr, PNPM_BLOCK.join("\n"));
}

#[test]
fn a_tool_that_is_not_pinned_is_not_checked() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        env(),
        FakeHttp::default().with_empty_advisories(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-yarn", "4.0.0"]));
    let (gate, stderr) = run(&context, &requested("v22.11.0", pnpm("9.1.0")));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(http.requests(), [DEFAULT_INDEX]);
}

#[test]
fn bundled_npm_of_an_installed_node_is_read_from_its_package_json() {
    let fs = FakeFileSystem::default().with_file(
        "/n/versions/node/v20.10.0/lib/node_modules/npm/package.json",
        r#"{ "name": "npm", "version": "10.1.0" }"#,
    );
    let (env, http) = (env(), FakeHttp::default().with_empty_advisories());
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-npm", "10.5.0"]));
    let (gate, stderr) = run(&context, &requested("v20.10.0", ProjectPins::default()));
    assert!(stopped(&gate), "{gate:?}");
    let expected = [
        "nvm: npm 10.1.0 (bundled with node v20.10.0) is not allowed by policy:",
        "  - below the minimum 10.5.0 (from --min-npm)",
        "Pin an allowed npm in .nvmrc (npm=<version>).",
    ];
    assert_eq!(stderr, expected.join("\n"));
    assert_eq!(http.requests(), [DEFAULT_INDEX]);
}

#[test]
fn bundled_npm_for_install_comes_from_the_index() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let http = FakeHttp::default().with_body(INDEX_URL, &index_text(&[("v20.10.0", "Iron")]));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let request = requested("v20.10.0", ProjectPins::default());
    let npm = bundled_npm(&context, &request).unwrap();
    assert_eq!(npm.to_string(), "v10.0.0");
    let offline = Requested {
        offline: true,
        ..request
    };
    let error = bundled_npm(&context, &offline).unwrap_err();
    assert!(
        error.to_string().ends_with("unavailable: offline"),
        "{error}"
    );
    assert_eq!(http.requests(), [INDEX_URL]);
}

#[test]
fn bundled_npm_is_not_looked_up_without_an_npm_rule() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        env(),
        FakeHttp::default().with_empty_advisories(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-pnpm", "9.12.0"]));
    let (gate, _) = run(&context, &requested("v20.10.0", ProjectPins::default()));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    assert_eq!(http.requests(), [DEFAULT_INDEX]);
}

/// A world whose policy requires pnpm releases at least 7 days old, on
/// 2026-10-17.
fn age_world() -> (FakeFileSystem, FakeEnv, FakeClock) {
    let fs = FakeFileSystem::default().with_file("/p.json", AGE_POLICY);
    let today = days_from_iso_date("2026-10-17").unwrap();
    let clock = FakeClock::at(today * 86_400 + 3_600);
    (fs, env().with_var("NVMRC_POLICY", "/p.json"), clock)
}

#[test]
fn an_age_rule_fetches_the_packument_and_fails_when_too_recent() {
    let (fs, env, clock) = age_world();
    let packument = r#"{ "time": { "9.12.0": "2026-10-15T08:00:00.000Z" } }"#;
    let http = FakeHttp::default()
        .with_empty_advisories()
        .with_body(PACKUMENT_URL, packument);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_clock(&clock);
    let (gate, stderr) = run(&context, &requested("v22.11.0", pnpm("9.12.0")));
    assert!(stopped(&gate), "{gate:?}");
    let expected = [
        "nvm: pnpm 9.12.0 (from /p/.nvmrc) is not allowed by policy:",
        "  - published 2 days ago; at least 7 required (from policy /p.json)",
        "Raise /p/.nvmrc to an allowed version.",
    ];
    assert_eq!(stderr, expected.join("\n"));
    assert_eq!(http.requests(), [DEFAULT_INDEX, PACKUMENT_URL]);
}

#[test]
fn an_unreachable_registry_with_an_age_rule_exits_9() {
    let (fs, env, clock) = age_world();
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_clock(&clock);
    let error = run(&context, &requested("v22.11.0", pnpm("9.12.0")))
        .0
        .unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(error.to_string().contains(PACKUMENT_URL), "{error}");
}

#[test]
fn offline_with_an_age_rule_exits_9() {
    let (fs, env, clock) = age_world();
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_clock(&clock);
    let request = Requested {
        offline: true,
        ..requested("v22.11.0", pnpm("9.12.0"))
    };
    let error = run(&context, &request).0.unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(
        error.to_string().ends_with("unavailable: offline"),
        "{error}"
    );
    assert!(!http.requests().iter().any(|url| url == PACKUMENT_URL));
}

#[test]
fn a_node_violation_with_an_unknown_tool_still_exits_7() {
    let (fs, env, clock) = age_world();
    let http = FakeHttp::default().with_empty_advisories();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_clock(&clock)
        .with_policy_flags(flags(&["--min-node", "22"]));
    let (gate, stderr) = run(&context, &requested("v20.10.0", pnpm("9.12.0")));
    assert!(stopped(&gate), "{gate:?}");
    let last = stderr.lines().last().unwrap_or_default();
    let note = "nvm: warning: cannot check pnpm: the release information is unavailable: ";
    assert_eq!(last, format!("{note}{PACKUMENT_URL}"), "{stderr}");
}

#[test]
fn node_and_pnpm_violations_are_reported_together() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        env(),
        FakeHttp::default().with_empty_advisories(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-node", "22", "--min-pnpm", "9.12.0"]));
    let request = Requested {
        origin: Origin::Nvmrc("/p/.nvmrc".into()),
        ..requested("v20.10.0", pnpm("9.1.0"))
    };
    let (gate, stderr) = run(&context, &request);
    assert!(stopped(&gate), "{gate:?}");
    let mut expected = vec![
        "nvm: node v20.10.0 (from /p/.nvmrc) is not allowed by policy:",
        "  - below the minimum v22.0.0 (from --min-node)",
        "Raise /p/.nvmrc to an allowed version.",
    ];
    expected.extend(PNPM_BLOCK);
    assert_eq!(stderr, expected.join("\n"));
}

#[test]
fn the_auto_path_warns_about_a_tool_and_passes() {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        env(),
        FakeHttp::default().with_empty_advisories(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(flags(&["--min-pnpm", "9.12.0"]))
        .advisory();
    let (gate, stderr) = run(&context, &requested("v22.11.0", pnpm("9.1.0")));
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    let first = stderr.lines().next().unwrap_or_default();
    assert_eq!(first, PNPM_BLOCK[0].replace("nvm: ", "nvm: warning: "));
    assert_eq!(stderr.lines().count(), 3, "{stderr}");
}
